// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Builder for fast state machine rules.

use std::collections::HashMap;

use qubit_fast_cas::FastCas;
use qubit_fast_cas::FastCasPolicy;

use super::FastStateMachine;
use super::FastStateMachineBuildError;

/// Default retry policy used by [`FastStateMachineBuilder`].
///
/// The default keeps construction lightweight and gives a reasonably balanced
/// fast-path retry budget for hot transition loops.
pub const FAST_STATE_MACHINE_DEFAULT_CAS_POLICY: FastCasPolicy = FastCasPolicy::spin(16);

/// Builder for dense, `u64`-coded state machine rules.
///
/// Counts define contiguous code ranges. State codes must be less than
/// `state_count`, event codes must be less than `event_count`, and the dense
/// transition table requires `state_count * event_count` cells.
#[must_use = "a fast state machine builder must be configured and built"]
#[derive(Debug, Clone)]
pub struct FastStateMachineBuilder {
    /// Configured number of state codes.
    state_count: Option<u64>,
    /// Configured number of event codes.
    event_count: Option<u64>,
    /// State codes marked as initial.
    initial_states: Vec<u64>,
    /// State codes marked as final.
    final_states: Vec<u64>,
    /// Configured `(source, event, target)` transition definitions.
    transitions: Vec<(u64, u64, u64)>,
    /// Retry policy used for runtime CAS conflicts.
    cas_policy: FastCasPolicy,
}

impl FastStateMachineBuilder {
    /// Creates an empty builder.
    ///
    /// # Returns
    /// A builder using [`FAST_STATE_MACHINE_DEFAULT_CAS_POLICY`] with no
    /// counts, state markers, or transitions configured.
    #[inline]
    pub fn new() -> Self {
        Self {
            state_count: None,
            event_count: None,
            initial_states: Vec::new(),
            final_states: Vec::new(),
            transitions: Vec::new(),
            cas_policy: FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
        }
    }

    /// Sets the number of state codes used by this machine.
    ///
    /// # Parameters
    /// - `count`: Exclusive upper bound for valid state codes.
    ///
    /// # Returns
    /// The updated builder.
    #[inline(always)]
    pub const fn state_count(mut self, count: u64) -> Self {
        self.state_count = Some(count);
        self
    }

    /// Sets the number of event codes used by this machine.
    ///
    /// # Parameters
    /// - `count`: Exclusive upper bound for valid event codes.
    ///
    /// # Returns
    /// The updated builder.
    #[inline(always)]
    pub const fn event_count(mut self, count: u64) -> Self {
        self.event_count = Some(count);
        self
    }

    /// Registers one initial state code.
    ///
    /// # Parameters
    /// - `state`: Initial state code, which must be less than `state_count`.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn initial_state(mut self, state: u64) -> Self {
        self.initial_states.push(state);
        self
    }

    /// Registers multiple initial state codes.
    ///
    /// # Parameters
    /// - `states`: Initial state codes, each of which must be less than
    ///   `state_count`.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn initial_states(mut self, states: &[u64]) -> Self {
        self.initial_states.extend(states.iter().copied());
        self
    }

    /// Registers one final state code.
    ///
    /// # Parameters
    /// - `state`: Final state code, which must be less than `state_count`.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn final_state(mut self, state: u64) -> Self {
        self.final_states.push(state);
        self
    }

    /// Registers multiple final state codes.
    ///
    /// # Parameters
    /// - `states`: Final state codes, each of which must be less than
    ///   `state_count`.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn final_states(mut self, states: &[u64]) -> Self {
        self.final_states.extend(states.iter().copied());
        self
    }

    /// Adds one transition by source state code, event code, and target state.
    ///
    /// Repeating the same `(source, event, target)` definition is allowed.
    /// Mapping one `(source, event)` pair to different targets is rejected by
    /// [`Self::build`].
    ///
    /// # Parameters
    /// - `source`: State code required before the event.
    /// - `event`: Event code that selects the transition.
    /// - `target`: State code installed after the transition.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn transition(mut self, source: u64, event: u64, target: u64) -> Self {
        self.transitions.push((source, event, target));
        self
    }

    /// Sets the retry policy used by [`FastStateMachine`] for CAS conflicts.
    ///
    /// # Parameters
    /// - `cas_policy`: Policy applied independently to every runtime trigger.
    ///
    /// # Returns
    /// The updated builder.
    #[inline(always)]
    pub const fn cas_policy(mut self, cas_policy: FastCasPolicy) -> Self {
        self.cas_policy = cas_policy;
        self
    }

    /// Builds and validates an immutable fast state machine.
    ///
    /// Configuration ranges and duplicate transitions are validated before the
    /// dense table is allocated.
    ///
    /// # Returns
    /// A state machine containing the validated dense transition table.
    ///
    /// # Errors
    /// Returns [`FastStateMachineBuildError::StateCountNotConfigured`] or
    /// [`FastStateMachineBuildError::EventCountNotConfigured`] when a required
    /// count is missing. Zero counts, out-of-range state/event codes,
    /// conflicting transitions, `u64` multiplication overflow, platform index
    /// limits, and allocation failure are reported by their corresponding
    /// [`FastStateMachineBuildError`] variants.
    pub fn build(self) -> Result<FastStateMachine, FastStateMachineBuildError> {
        let state_count = self
            .state_count
            .ok_or(FastStateMachineBuildError::StateCountNotConfigured)?;
        let event_count = self
            .event_count
            .ok_or(FastStateMachineBuildError::EventCountNotConfigured)?;

        if state_count == 0 {
            return Err(FastStateMachineBuildError::InvalidStateCount { count: state_count });
        }
        if event_count == 0 {
            return Err(FastStateMachineBuildError::InvalidEventCount { count: event_count });
        }

        self.validate_state_sets(state_count)?;
        self.validate_transitions(state_count, event_count)?;

        let transition_count =
            state_count
                .checked_mul(event_count)
                .ok_or(FastStateMachineBuildError::TransitionTableOverflow {
                    state_count,
                    event_count,
                })?;
        let transition_capacity = Self::storage_capacity(transition_count, state_count, event_count)?;
        let state_capacity = Self::storage_capacity(state_count, state_count, event_count)?;

        let mut transitions = Self::allocate_filled(transition_capacity, u64::MAX, state_count, event_count)?;
        for &(source, event, target) in &self.transitions {
            let index = source * event_count + event;
            let index = Self::storage_capacity(index, state_count, event_count)?;
            transitions[index] = target;
        }

        let mut initial_states = Self::allocate_filled(state_capacity, false, state_count, event_count)?;
        for state in self.initial_states {
            let index = Self::storage_capacity(state, state_count, event_count)?;
            initial_states[index] = true;
        }

        let mut final_states = Self::allocate_filled(state_capacity, false, state_count, event_count)?;
        for state in self.final_states {
            let index = Self::storage_capacity(state, state_count, event_count)?;
            final_states[index] = true;
        }

        Ok(FastStateMachine {
            state_count,
            event_count,
            initial_states,
            final_states,
            transitions,
            cas: FastCas::with_policy(self.cas_policy),
        })
    }

    /// Validates every configured initial and final state before allocating the
    /// dense transition table.
    ///
    /// # Parameters
    /// - `state_count`: Exclusive upper bound for state codes.
    ///
    /// # Returns
    /// `Ok(())` when every configured marker is in range.
    ///
    /// # Errors
    /// Returns the corresponding initial- or final-state range error for the
    /// first invalid code.
    fn validate_state_sets(&self, state_count: u64) -> Result<(), FastStateMachineBuildError> {
        for &state in &self.initial_states {
            if state >= state_count {
                return Err(FastStateMachineBuildError::InitialStateOutOfRange { state, state_count });
            }
        }
        for &state in &self.final_states {
            if state >= state_count {
                return Err(FastStateMachineBuildError::FinalStateOutOfRange { state, state_count });
            }
        }
        Ok(())
    }

    /// Validates transition ranges and conflicting duplicate definitions before
    /// allocating the dense transition table.
    ///
    /// # Parameters
    /// - `state_count`: Exclusive upper bound for source and target state
    ///   codes.
    /// - `event_count`: Exclusive upper bound for event codes.
    ///
    /// # Returns
    /// `Ok(())` when every transition is in range and deterministic.
    ///
    /// # Errors
    /// Returns a range error, duplicate-transition error, or capacity error if
    /// the temporary duplicate-detection map cannot reserve storage.
    fn validate_transitions(&self, state_count: u64, event_count: u64) -> Result<(), FastStateMachineBuildError> {
        let mut targets = HashMap::new();
        if targets.try_reserve(self.transitions.len()).is_err() {
            return Err(Self::capacity_error(state_count, event_count));
        }

        for &(source, event, target) in &self.transitions {
            if source >= state_count {
                return Err(FastStateMachineBuildError::TransitionSourceOutOfRange {
                    source_state: source,
                    state_count,
                });
            }
            if event >= event_count {
                return Err(FastStateMachineBuildError::TransitionEventOutOfRange { event, event_count });
            }
            if target >= state_count {
                return Err(FastStateMachineBuildError::TransitionTargetOutOfRange { target, state_count });
            }

            if let Some(&existing_target) = targets.get(&(source, event)) {
                if existing_target != target {
                    return Err(FastStateMachineBuildError::DuplicateTransition {
                        source_state: source,
                        event,
                        existing_target,
                        new_target: target,
                    });
                }
            } else {
                targets.insert((source, event), target);
            }
        }
        Ok(())
    }

    /// Creates a capacity error carrying the configured table dimensions.
    ///
    /// # Parameters
    /// - `state_count`: Configured state count.
    /// - `event_count`: Configured event count.
    ///
    /// # Returns
    /// A capacity error for the configured dense table.
    #[inline(always)]
    const fn capacity_error(state_count: u64, event_count: u64) -> FastStateMachineBuildError {
        FastStateMachineBuildError::TransitionTableCapacityExceeded {
            state_count,
            event_count,
        }
    }

    /// Converts a validated `u64` length or index into the platform's storage
    /// index type.
    ///
    /// # Parameters
    /// - `value`: Length or index to convert.
    /// - `state_count`: State count included in a capacity error.
    /// - `event_count`: Event count included in a capacity error.
    ///
    /// # Returns
    /// The equivalent `usize` value.
    ///
    /// # Errors
    /// Returns a capacity error when the current platform cannot represent
    /// `value` as `usize`.
    #[inline]
    fn storage_capacity(value: u64, state_count: u64, event_count: u64) -> Result<usize, FastStateMachineBuildError> {
        match usize::try_from(value) {
            Ok(capacity) => Ok(capacity),
            Err(_) => Err(Self::capacity_error(state_count, event_count)),
        }
    }

    /// Allocates and fills a vector without using an infallible capacity
    /// reservation.
    ///
    /// # Type Parameters
    /// - `T`: Cloneable cell value stored in the vector.
    ///
    /// # Parameters
    /// - `capacity`: Exact number of cells to allocate.
    /// - `value`: Initial value cloned into every cell.
    /// - `state_count`: State count included in a capacity error.
    /// - `event_count`: Event count included in a capacity error.
    ///
    /// # Returns
    /// A vector with `capacity` initialized cells.
    ///
    /// # Errors
    /// Returns a capacity error if reservation fails.
    fn allocate_filled<T: Clone>(
        capacity: usize,
        value: T,
        state_count: u64,
        event_count: u64,
    ) -> Result<Vec<T>, FastStateMachineBuildError> {
        let mut values = Vec::new();
        if values.try_reserve_exact(capacity).is_err() {
            return Err(Self::capacity_error(state_count, event_count));
        }
        values.resize(capacity, value);
        Ok(values)
    }
}

impl Default for FastStateMachineBuilder {
    /// Creates the same empty builder as [`Self::new`].
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}
