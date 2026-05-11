/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

//! Builder for fast state machine rules.

use crate::FastCasPolicy;

use super::{
    FastStateMachine,
    FastStateMachineBuildError,
};
use qubit_cas::FastCas;

/// Default retry policy used by [`FastStateMachineBuilder`].
///
/// The default keeps construction lightweight and gives a reasonably balanced
/// fast-path retry budget for hot transition loops.
pub const FAST_STATE_MACHINE_DEFAULT_CAS_POLICY: FastCasPolicy = FastCasPolicy::spin(16);

/// Builder for dense, integer-coded state machine rules.
#[derive(Debug, Clone)]
pub struct FastStateMachineBuilder {
    state_count: Option<usize>,
    event_count: Option<usize>,
    initial_states: Vec<usize>,
    final_states: Vec<usize>,
    transitions: Vec<(usize, usize, usize)>,
    cas_policy: FastCasPolicy,
}

impl FastStateMachineBuilder {
    /// Creates an empty builder.
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
    pub const fn state_count(mut self, count: usize) -> Self {
        self.state_count = Some(count);
        self
    }

    /// Sets the number of event codes used by this machine.
    pub const fn event_count(mut self, count: usize) -> Self {
        self.event_count = Some(count);
        self
    }

    /// Registers one initial state code.
    pub fn initial_state(mut self, state: usize) -> Self {
        self.initial_states.push(state);
        self
    }

    /// Registers multiple initial state codes.
    pub fn initial_states(mut self, states: &[usize]) -> Self {
        self.initial_states.extend(states.iter().copied());
        self
    }

    /// Registers one final state code.
    pub fn final_state(mut self, state: usize) -> Self {
        self.final_states.push(state);
        self
    }

    /// Registers multiple final state codes.
    pub fn final_states(mut self, states: &[usize]) -> Self {
        self.final_states.extend(states.iter().copied());
        self
    }

    /// Adds one transition by source state code, event code, and target state.
    pub fn transition(mut self, source: usize, event: usize, target: usize) -> Self {
        self.transitions.push((source, event, target));
        self
    }

    /// Sets the retry policy used by [`FastStateMachine`] for CAS conflicts.
    pub const fn cas_policy(mut self, cas_policy: FastCasPolicy) -> Self {
        self.cas_policy = cas_policy;
        self
    }

    /// Builds and validates an immutable fast state machine.
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

        let transition_count = state_count.checked_mul(event_count).ok_or(
            FastStateMachineBuildError::TransitionTableOverflow {
                state_count,
                event_count,
            },
        )?;

        let mut initial_states = vec![false; state_count];
        for state in self.initial_states {
            if state >= state_count {
                return Err(FastStateMachineBuildError::InitialStateOutOfRange {
                    state,
                    state_count,
                });
            }
            initial_states[state] = true;
        }

        let mut final_states = vec![false; state_count];
        for state in self.final_states {
            if state >= state_count {
                return Err(FastStateMachineBuildError::FinalStateOutOfRange {
                    state,
                    state_count,
                });
            }
            final_states[state] = true;
        }

        let mut transitions = vec![usize::MAX; transition_count];
        for (source, event, target) in self.transitions {
            if source >= state_count {
                return Err(FastStateMachineBuildError::TransitionSourceOutOfRange {
                    source_state: source,
                    state_count,
                });
            }
            if event >= event_count {
                return Err(FastStateMachineBuildError::TransitionEventOutOfRange {
                    event,
                    event_count,
                });
            }
            if target >= state_count {
                return Err(FastStateMachineBuildError::TransitionTargetOutOfRange {
                    target,
                    state_count,
                });
            }

            let index = source
                .checked_mul(event_count)
                .and_then(|base| base.checked_add(event))
                .expect("validated transition index must fit usize");

            match transitions[index] {
                usize::MAX => transitions[index] = target,
                existing if existing == target => {}
                existing => {
                    return Err(FastStateMachineBuildError::DuplicateTransition {
                        source_state: source,
                        event,
                        existing_target: existing,
                        new_target: target,
                    });
                }
            }
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
}

impl Default for FastStateMachineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
