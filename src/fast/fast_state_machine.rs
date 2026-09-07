// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Integer-coded state machine implementation.

use qubit_fast_cas::FastCas;
use qubit_fast_cas::FastCasDecision;
use qubit_fast_cas::FastCasPolicy;
use qubit_fast_cas::FastCasState;

use super::FastStateMachineError;
use super::FastStateMachineResult;
use super::fast_state_machine_error::fast_state_machine_error_from_fast_cas_error;

/// Sentinel stored in dense table cells without a configured transition.
const UNSET_TRANSITION: u64 = u64::MAX;

/// A compact, high-performance state machine backed by [`FastCas`].
///
/// States and events are represented as contiguous integer code spaces:
///
/// - states are in `[0, state_count)`
/// - events are in `[0, event_count)`
///
/// Transition resolution is a single table index lookup:
/// `index = source * event_count + event`.
#[must_use = "a fast state machine contains the configured transition rules"]
#[derive(Debug, Clone)]
pub struct FastStateMachine {
    /// Number of valid state codes.
    pub(super) state_count: u64,
    /// Number of valid event codes.
    pub(super) event_count: u64,
    /// Dense flags indexed by validated state code.
    pub(super) initial_states: Vec<bool>,
    /// Dense flags indexed by validated state code.
    pub(super) final_states: Vec<bool>,
    /// Row-major transition targets, using [`UNSET_TRANSITION`] for empty
    /// cells.
    pub(super) transitions: Vec<u64>,
    /// Policy-driven CAS executor used by runtime transitions.
    pub(super) cas: FastCas,
}

impl FastStateMachine {
    /// Creates a builder used to configure counts, initial/final flags,
    /// transitions, and CAS policy before calling
    /// [`super::FastStateMachineBuilder::build`].
    ///
    /// # Returns
    /// A new, empty [`super::FastStateMachineBuilder`].
    #[inline(always)]
    pub fn builder() -> super::FastStateMachineBuilder {
        super::FastStateMachineBuilder::new()
    }

    /// Returns the number of distinct state codes configured for this machine.
    ///
    /// Valid state codes are integers in `0..state_count()`.
    ///
    /// # Returns
    /// The configured state-space size (length of the transition table rows).
    #[inline(always)]
    pub const fn state_count(&self) -> u64 {
        self.state_count
    }

    /// Returns the number of distinct event codes accepted by this machine.
    ///
    /// Valid event codes are integers in `0..event_count()`.
    ///
    /// # Returns
    /// The configured event-space size (length of each row in the transition
    /// table).
    #[inline(always)]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }

    /// Returns the dense transition table.
    ///
    /// The table is laid out row-major: source index first, then event index.
    /// Cells without a configured transition contain `u64::MAX`; prefer
    /// [`Self::transition_target`] when the sentinel is not needed.
    ///
    /// # Returns
    /// The immutable row-major transition cells.
    #[inline(always)]
    pub fn transitions(&self) -> &[u64] {
        &self.transitions
    }

    /// Returns the CAS retry policy used for all transitions.
    ///
    /// This is the policy configured in the builder via
    /// [`crate::FastStateMachineBuilder::cas_policy`], or
    /// [`crate::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY`] when no override is
    /// supplied.
    ///
    /// # Returns
    /// The configured Fast CAS policy.
    #[inline(always)]
    pub fn cas_policy(&self) -> FastCasPolicy {
        self.cas.policy()
    }

    /// Returns a read-only slice marking which state codes are initial.
    ///
    /// The slice has length [`Self::state_count`]; index `s` corresponds to
    /// state code `s`, and is `true` if that state was registered as
    /// initial in the builder.
    ///
    /// # Returns
    /// Dense initial-state flags in state-code order.
    #[inline(always)]
    pub fn initial_states(&self) -> &[bool] {
        &self.initial_states
    }

    /// Returns a read-only slice marking which state codes are final
    /// (accepting).
    ///
    /// The slice has length [`Self::state_count`]; index `s` corresponds to
    /// state code `s`, and is `true` if that state was registered as final
    /// in the builder.
    ///
    /// # Returns
    /// Dense final-state flags in state-code order.
    #[inline(always)]
    pub fn final_states(&self) -> &[bool] {
        &self.final_states
    }

    /// Returns whether `state` is a valid code for this machine.
    ///
    /// # Parameters
    /// - `state`: Candidate state code.
    ///
    /// # Returns
    /// `true` if `state < state_count()`, otherwise `false`.
    #[inline(always)]
    pub const fn contains_state(&self, state: u64) -> bool {
        state < self.state_count
    }

    /// Returns whether `state` was configured as an initial state.
    ///
    /// # Parameters
    /// - `state`: State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked initial; `false` if out of
    /// range or not initial.
    #[inline]
    pub fn is_initial_state(&self, state: u64) -> bool {
        self.state_index(state)
            .and_then(|index| self.initial_states.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// Returns whether `state` was configured as a final state.
    ///
    /// # Parameters
    /// - `state`: State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked final; `false` if out of range
    /// or not final.
    #[inline]
    pub fn is_final_state(&self, state: u64) -> bool {
        self.state_index(state)
            .and_then(|index| self.final_states.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// Looks up the next state for a specific `(source, event)` pair.
    ///
    /// # Parameters
    /// - `source`: Current state code.
    /// - `event`: Event code.
    ///
    /// # Returns
    /// `Some(target)` when a transition is configured; `None` if `source` or
    /// `event` is out of range, or if no transition exists for that pair.
    #[inline]
    pub fn transition_target(&self, source: u64, event: u64) -> Option<u64> {
        if !self.contains_state(source) {
            return None;
        }
        self.transition_target_for_valid_state(source, event)
    }

    /// Resolves a transition after the caller has validated the source state.
    ///
    /// # Parameters
    /// - `source`: Source state known to be less than [`Self::state_count`].
    /// - `event`: Event code to resolve.
    ///
    /// # Returns
    /// The configured target, or `None` when `event` is out of range or the
    /// transition cell is unset.
    #[inline]
    fn transition_target_for_valid_state(&self, source: u64, event: u64) -> Option<u64> {
        let index = self.transition_index(source, event)?;
        self.transitions
            .get(index)
            .copied()
            .filter(|&target| target != UNSET_TRANSITION)
    }

    /// Applies one event atomically on `state` using the configured [`FastCas`]
    /// policy.
    ///
    /// Reads the current code from `state`, resolves the transition for
    /// `event`, and stores the new code back if the transition is valid.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// `Ok(new_state)` after a successful transition and CAS store.
    ///
    /// # Errors
    /// * [`FastStateMachineError::UnknownState`] — current code not in this
    ///   machine.
    /// * [`FastStateMachineError::UnknownTransition`] — no transition for
    ///   `(current, event)`.
    /// * [`FastStateMachineError::CasConflict`] — CAS retries exhausted.
    #[inline(always)]
    pub fn trigger(&self, state: &FastCasState, event: u64) -> FastStateMachineResult {
        let (_old_state, new_state) = self.change_state(state, event)?;
        Ok(new_state)
    }

    /// Like [`Self::trigger`], but invokes `on_success` after the CAS update
    /// succeeds.
    ///
    /// The callback receives `(old_state, new_state)` as observed for the
    /// successful transition. It is not called when [`Self::trigger`] would
    /// return an error.
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    /// - `on_success`: Called with previous and new state codes only on
    ///   success.
    ///
    /// # Returns
    /// Same as [`Self::trigger`]: `Ok(new_state)` on success.
    ///
    /// # Errors
    /// Same as [`Self::trigger`].
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    #[inline]
    pub fn trigger_with<F>(&self, state: &FastCasState, event: u64, on_success: F) -> FastStateMachineResult
    where
        F: FnOnce(u64, u64),
    {
        let (old_state, new_state) = self.change_state(state, event)?;
        on_success(old_state, new_state);
        Ok(new_state)
    }

    /// Attempts the same transition as [`Self::trigger`], discarding error
    /// details.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// `true` if the transition was committed; `false` if validation failed or
    /// CAS retries were exhausted. A successful self-transition also returns
    /// `true`.
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline(always)]
    pub fn try_trigger(&self, state: &FastCasState, event: u64) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Like [`Self::trigger_with`], but returns only whether the transition
    /// succeeded.
    ///
    /// `on_success` runs only when the CAS update succeeds, matching
    /// [`Self::trigger_with`].
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    /// - `on_success`: Called with previous and new state codes only on
    ///   success.
    ///
    /// # Returns
    /// `true` when the transition commits; `false` on any error that
    /// [`Self::trigger_with`] would surface.
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline(always)]
    pub fn try_trigger_with<F>(&self, state: &FastCasState, event: u64, on_success: F) -> bool
    where
        F: FnOnce(u64, u64),
    {
        self.trigger_with(state, event, on_success).is_ok()
    }

    /// Runs one CAS-backed transition: validates `event` against the loaded
    /// current code and installs the next code or aborts with
    /// [`FastStateMachineError`].
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// The previous and committed state codes.
    ///
    /// # Errors
    /// Returns an unknown-state, unknown-transition, or exhausted-conflict
    /// error.
    fn change_state(&self, state: &FastCasState, event: u64) -> Result<(u64, u64), FastStateMachineError> {
        match self.cas.execute::<u64, FastStateMachineError, _>(state, |current| {
            match self.next_state(current, event) {
                Ok(new_state) => FastCasDecision::update(new_state, new_state),
                Err(error) => FastCasDecision::abort(error),
            }
        }) {
            Ok(success) => Ok((success.previous(), success.current())),
            Err(error) => Err(fast_state_machine_error_from_fast_cas_error(error)),
        }
    }

    /// Validates `state` and resolves the successor for `event` using the
    /// transition table.
    ///
    /// # Parameters
    /// - `state`: Current state code.
    /// - `event`: Event code to resolve.
    ///
    /// # Returns
    /// The configured target state.
    ///
    /// # Errors
    /// Returns [`FastStateMachineError::UnknownState`] for an out-of-range
    /// state, or [`FastStateMachineError::UnknownTransition`] when no target is
    /// configured for the pair.
    #[inline]
    fn next_state(&self, state: u64, event: u64) -> Result<u64, FastStateMachineError> {
        if !self.contains_state(state) {
            return Err(FastStateMachineError::UnknownState { state });
        }

        self.transition_target_for_valid_state(state, event)
            .ok_or(FastStateMachineError::UnknownTransition {
                source_state: state,
                event,
            })
    }

    /// Converts an in-range state code into a slice index.
    ///
    /// # Parameters
    /// - `state`: Candidate state code.
    ///
    /// # Returns
    /// The platform index for an in-range code, or `None` otherwise.
    #[inline]
    fn state_index(&self, state: u64) -> Option<usize> {
        if !self.contains_state(state) {
            return None;
        }
        usize::try_from(state).ok()
    }

    /// Computes the row-major table index for a validated state and event pair.
    ///
    /// # Parameters
    /// - `source`: Source state already known to be in range.
    /// - `event`: Candidate event code.
    ///
    /// # Returns
    /// The platform table index, or `None` when `event` or the computed index
    /// is not representable.
    #[inline]
    fn transition_index(&self, source: u64, event: u64) -> Option<usize> {
        if event >= self.event_count {
            return None;
        }
        let index = source.checked_mul(self.event_count)?.checked_add(event)?;
        usize::try_from(index).ok()
    }
}
