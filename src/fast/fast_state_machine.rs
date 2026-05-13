/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

//! Integer-coded state machine implementation.

use qubit_cas::{
    FastCas,
    FastCasDecision,
    FastCasError,
    FastCasState,
};

use super::{
    FastStateMachineError,
    FastStateMachineResult,
};

const UNSET_TRANSITION: usize = usize::MAX;

/// A compact, high-performance state machine backed by [`FastCas`].
///
/// States and events are represented as contiguous integer code spaces:
///
/// - states are in `[0, state_count)`
/// - events are in `[0, event_count)`
///
/// Transition resolution is a single table index lookup:
/// `index = source * event_count + event`.
#[derive(Debug, Clone)]
pub struct FastStateMachine {
    pub(super) state_count: usize,
    pub(super) event_count: usize,
    pub(super) initial_states: Vec<bool>,
    pub(super) final_states: Vec<bool>,
    pub(super) transitions: Vec<usize>,
    pub(super) cas: FastCas,
}

impl FastStateMachine {
    /// Creates a builder used to configure counts, initial/final flags, transitions,
    /// and CAS policy before calling [`super::FastStateMachineBuilder::build`].
    ///
    /// # Returns
    /// A new, empty [`super::FastStateMachineBuilder`].
    pub fn builder() -> super::FastStateMachineBuilder {
        super::FastStateMachineBuilder::new()
    }

    /// Returns the number of distinct state codes configured for this machine.
    ///
    /// Valid state codes are integers in `0..state_count()`.
    ///
    /// # Returns
    /// The configured state-space size (length of the transition table rows).
    pub const fn state_count(&self) -> usize {
        self.state_count
    }

    /// Returns the number of distinct event codes accepted by this machine.
    ///
    /// Valid event codes are integers in `0..event_count()`.
    ///
    /// # Returns
    /// The configured event-space size (length of each row in the transition table).
    pub const fn event_count(&self) -> usize {
        self.event_count
    }

    /// Returns the dense transition table.
    ///
    /// The table is laid out row-major: source index first, then event index.
    pub fn transitions(&self) -> &[usize] {
        &self.transitions
    }

    /// Returns the CAS retry policy used for all transitions.
    ///
    /// This is the policy configured in the builder via
    /// [`crate::FastStateMachineBuilder::cas_policy`], or
    /// [`crate::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY`] when no override is
    /// supplied.
    pub fn cas_policy(&self) -> qubit_cas::FastCasPolicy {
        self.cas.policy()
    }

    /// Returns a read-only slice marking which state codes are initial.
    ///
    /// The slice has length [`Self::state_count`]; index `s` corresponds to state code `s`,
    /// and is `true` if that state was registered as initial in the builder.
    pub fn initial_states(&self) -> &[bool] {
        &self.initial_states
    }

    /// Returns a read-only slice marking which state codes are final (accepting).
    ///
    /// The slice has length [`Self::state_count`]; index `s` corresponds to state code `s`,
    /// and is `true` if that state was registered as final in the builder.
    pub fn final_states(&self) -> &[bool] {
        &self.final_states
    }

    /// Returns whether `state` is a valid code for this machine.
    ///
    /// # Arguments
    /// * `state` — Candidate state code.
    ///
    /// # Returns
    /// `true` if `state < state_count()`, otherwise `false`.
    pub const fn contains_state(&self, state: usize) -> bool {
        state < self.state_count
    }

    /// Returns whether `state` was configured as an initial state.
    ///
    /// # Arguments
    /// * `state` — State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked initial; `false` if out of range or not initial.
    pub fn is_initial_state(&self, state: usize) -> bool {
        self.initial_states.get(state).copied().unwrap_or(false)
    }

    /// Returns whether `state` was configured as a final state.
    ///
    /// # Arguments
    /// * `state` — State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked final; `false` if out of range or not final.
    pub fn is_final_state(&self, state: usize) -> bool {
        self.final_states.get(state).copied().unwrap_or(false)
    }

    /// Looks up the next state for a specific `(source, event)` pair.
    ///
    /// # Arguments
    /// * `source` — Current state code.
    /// * `event` — Event code.
    ///
    /// # Returns
    /// `Some(target)` when a transition is configured; `None` if `source` or `event` is
    /// out of range, or if no transition exists for that pair.
    pub fn transition_target(&self, source: usize, event: usize) -> Option<usize> {
        self.get_transition_target(source, event)
            .filter(|&target| target != UNSET_TRANSITION)
    }

    /// Returns the raw table cell for `(source, event)` without treating the unset sentinel.
    ///
    /// Unlike [`Self::transition_target`], this does not map the internal “no transition”
    /// sentinel to `None`; callers that need a public [`Option`] should use
    /// [`Self::transition_target`].
    ///
    /// # Returns
    /// `None` when `source` or `event` is out of range; otherwise `Some(cell)` where `cell`
    /// may still denote “unset” in the packed table.
    fn get_transition_target(&self, source: usize, event: usize) -> Option<usize> {
        if !self.contains_state(source) || event >= self.event_count {
            return None;
        }
        let index = source
            .checked_mul(self.event_count)
            .and_then(|base| base.checked_add(event));
        index.map(|index| self.transitions[index])
    }

    /// Applies one event atomically on `state` using the configured [`FastCas`] policy.
    ///
    /// Reads the current code from `state`, resolves the transition for `event`, and stores
    /// the new code back if the transition is valid.
    ///
    /// # Arguments
    /// * `state` — Shared compact state updated by compare-and-swap.
    /// * `event` — Event code to apply.
    ///
    /// # Returns
    /// `Ok(new_state)` after a successful transition and CAS store.
    ///
    /// # Errors
    /// * [`FastStateMachineError::UnknownState`] — current code not in this machine.
    /// * [`FastStateMachineError::UnknownTransition`] — no transition for `(current, event)`.
    /// * [`FastStateMachineError::CasConflict`] — CAS retries exhausted.
    pub fn trigger(&self, state: &FastCasState, event: usize) -> FastStateMachineResult {
        let (_old_state, new_state) = self.change_state(state, event)?;
        Ok(new_state)
    }

    /// Like [`Self::trigger`], but invokes `on_success` after the CAS update succeeds.
    ///
    /// The callback receives `(old_state, new_state)` as observed for the successful
    /// transition. It is not called when [`Self::trigger`] would return an error.
    ///
    /// # Arguments
    /// * `state` — Shared compact state updated by compare-and-swap.
    /// * `event` — Event code to apply.
    /// * `on_success` — Called with previous and new state codes only on success.
    ///
    /// # Returns
    /// Same as [`Self::trigger`]: `Ok(new_state)` on success.
    ///
    /// # Errors
    /// Same as [`Self::trigger`].
    pub fn trigger_with<F>(
        &self,
        state: &FastCasState,
        event: usize,
        on_success: F,
    ) -> FastStateMachineResult
    where
        F: Fn(usize, usize),
    {
        let (old_state, new_state) = self.change_state(state, event)?;
        on_success(old_state, new_state);
        Ok(new_state)
    }

    /// Attempts the same transition as [`Self::trigger`], discarding error details.
    ///
    /// # Returns
    /// `true` if the transition and CAS update succeeded; `false` if validation failed or
    /// CAS retries were exhausted.
    pub fn try_trigger(&self, state: &FastCasState, event: usize) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Like [`Self::trigger_with`], but returns only whether the transition succeeded.
    ///
    /// `on_success` runs only when the CAS update succeeds, matching [`Self::trigger_with`].
    ///
    /// # Returns
    /// `true` on success; `false` on any error that [`Self::trigger_with`] would surface.
    pub fn try_trigger_with<F>(&self, state: &FastCasState, event: usize, on_success: F) -> bool
    where
        F: Fn(usize, usize),
    {
        self.trigger_with(state, event, on_success).is_ok()
    }

    /// Runs one CAS-backed transition: validates `event` against the loaded current code and
    /// installs the next code or aborts with [`FastStateMachineError`].
    fn change_state(
        &self,
        state: &FastCasState,
        event: usize,
    ) -> Result<(usize, usize), FastStateMachineError> {
        match self
            .cas
            .execute::<usize, FastStateMachineError, _>(state, |current| {
                match self.next_state(current, event) {
                    Ok(new_state) => FastCasDecision::update(new_state, new_state),
                    Err(error) => FastCasDecision::abort(error),
                }
            }) {
            Ok(success) => Ok((success.previous(), success.current())),
            Err(error) => Err(Self::state_error_from_cas_error(error)),
        }
    }

    /// Validates `state` and resolves the successor for `event` using the transition table.
    fn next_state(&self, state: usize, event: usize) -> Result<usize, FastStateMachineError> {
        if !self.contains_state(state) {
            return Err(FastStateMachineError::UnknownState { state });
        }

        self.transition_target(state, event)
            .ok_or(FastStateMachineError::UnknownTransition {
                source_state: state,
                event,
            })
    }

    /// Converts [`FastCasError`] into [`FastStateMachineError`] for public APIs.
    fn state_error_from_cas_error(
        error: FastCasError<FastStateMachineError>,
    ) -> FastStateMachineError {
        match error {
            FastCasError::Abort { error, .. } => error,
            FastCasError::Conflict { attempts, .. } => {
                FastStateMachineError::CasConflict { attempts }
            }
        }
    }
}
