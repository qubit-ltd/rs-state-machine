// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Typed configuration delegated to the validated dense integer builder.
use std::marker::PhantomData;

use qubit_fast_cas::FastCasPolicy;

use super::DenseCode;
use super::TypedFastStateMachine;
use super::TypedFastStateMachineBuildError;
use super::dense_code::checked_code;
use super::dense_code::validate_values;
use crate::FastStateMachineBuilder;
use crate::Transition;

/// Builds immutable dense rules using finite state `S` and event `E` values.
///
/// Counts come from their value tables. The first invalid input is retained;
/// encoding tables are validated before rule validation and dense allocation.
///
/// # Type Parameters
/// - `S`: Finite state type implementing [`DenseCode`].
/// - `E`: Finite event type implementing [`DenseCode`].
///
/// # Examples
///
/// ```
/// use qubit_state_machine::{DenseCode, TypedFastStateMachineBuilder};
///
/// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// enum Event { Tick }
/// impl DenseCode for Event {
///     const VALUES: &'static [Self] = &[Self::Tick];
///     fn code(self) -> u64 { 0 }
/// }
/// let machine = TypedFastStateMachineBuilder::<State, Event>::new()
///     .initial_state(State::Ready).build().expect("valid rules");
/// assert_eq!(machine.initial_state(), State::Ready);
/// ```
#[must_use = "configure and build the typed state machine"]
#[derive(Debug, Clone)]
pub struct TypedFastStateMachineBuilder<S: DenseCode, E: DenseCode> {
    /// Shared raw builder, including CAS policy and pending transition rules.
    raw: FastStateMachineBuilder,
    /// First supplied value that failed its finite-table membership check.
    input_error: Option<TypedFastStateMachineBuildError>,
    /// State and event types with no runtime storage.
    marker: PhantomData<fn() -> (S, E)>,
}

impl<S: DenseCode, E: DenseCode> TypedFastStateMachineBuilder<S, E> {
    /// Creates an empty typed definition using counts from the value tables.
    ///
    /// # Returns
    /// A builder with the default Fast CAS policy and no configured initial
    /// state.
    pub fn new() -> Self {
        Self {
            raw: FastStateMachineBuilder::new()
                .state_count(S::VALUES.len() as u64)
                .event_count(E::VALUES.len() as u64),
            input_error: None,
            marker: PhantomData,
        }
    }

    /// Encodes an input, retaining the first invalid input for `build`.
    ///
    /// # Parameters
    /// - `value`: State or event to check.
    /// - `domain`: Either `state` or `event` for diagnostics.
    ///
    /// # Returns
    /// `Some(code)` for valid membership, or `None` after recording an error.
    fn encode<T: DenseCode>(&mut self, value: T, domain: &'static str) -> Option<u64> {
        match checked_code(value) {
            Some(code) => Some(code),
            None => {
                if self.input_error.is_none() {
                    self.input_error = Some(TypedFastStateMachineBuildError::ValueNotInCodebook {
                        domain,
                        code: value.code(),
                    });
                }
                None
            }
        }
    }

    /// Sets the unique initial `state`; the last valid setting wins.
    ///
    /// # Parameters
    /// - `state`: Initial state value.
    ///
    /// # Returns
    /// The updated builder. Invalid membership is reported by `build`.
    pub fn initial_state(mut self, state: S) -> Self {
        if let Some(code) = self.encode(state, "state") {
            self.raw = self.raw.initial_state(code);
        }
        self
    }

    /// Marks `state` terminal, forbidding all outgoing transitions.
    ///
    /// # Parameters
    /// - `state`: Terminal state value.
    ///
    /// # Returns
    /// The updated builder. Invalid membership is reported by `build`.
    pub fn terminal_state(mut self, state: S) -> Self {
        if let Some(code) = self.encode(state, "state") {
            self.raw = self.raw.terminal_state(code);
        }
        self
    }

    /// Marks every value in `states` terminal.
    ///
    /// # Parameters
    /// - `states`: Terminal state values.
    ///
    /// # Returns
    /// The updated builder, retaining the first invalid member for `build`.
    pub fn terminal_states(mut self, states: &[S]) -> Self {
        for &state in states {
            self = self.terminal_state(state);
        }
        self
    }

    /// Defines the target of `event` when the current state is `source`.
    ///
    /// # Parameters
    /// - `source`: State before the event.
    /// - `event`: Event selecting this transition.
    /// - `target`: State to commit after success.
    ///
    /// # Returns
    /// The updated builder. Invalid values and conflicting targets fail
    /// `build`.
    pub fn transition(mut self, source: S, event: E, target: S) -> Self {
        let source = self.encode(source, "state");
        let event = self.encode(event, "event");
        let target = self.encode(target, "state");
        if let (Some(source), Some(event), Some(target)) = (source, event, target) {
            self.raw = self.raw.transition(source, event, target);
        }
        self
    }

    /// Adds the supplied transition `value`.
    ///
    /// # Parameters
    /// - `value`: Transition value to add.
    ///
    /// # Returns
    /// The updated builder, with the same validation as `transition`.
    pub fn transition_value(self, value: Transition<S, E>) -> Self {
        self.transition(value.source(), value.event(), value.target())
    }

    /// Selects `policy` for each runtime CAS operation.
    ///
    /// # Parameters
    /// - `policy`: Fast CAS retry policy.
    ///
    /// # Returns
    /// The updated builder; the default is the original Fast engine's policy.
    pub fn cas_policy(mut self, policy: FastCasPolicy) -> Self {
        self.raw = self.raw.cas_policy(policy);
        self
    }

    /// Validates encoding tables and constructs the shared dense engine.
    ///
    /// # Returns
    /// Immutable typed rules, ready to create independent state cells.
    ///
    /// # Errors
    /// Checks state and event codebooks first, then the first invalid supplied
    /// value, then raw builder errors such as missing initial state, terminal
    /// outgoing edges, conflicting targets, overflow, and allocation failure.
    pub fn build(self) -> Result<TypedFastStateMachine<S, E>, TypedFastStateMachineBuildError> {
        validate_values::<S>("state")?;
        validate_values::<E>("event")?;
        if let Some(error) = self.input_error {
            return Err(error);
        }
        Ok(TypedFastStateMachine {
            raw: self.raw.build()?,
            marker: PhantomData,
        })
    }
}

impl<S: DenseCode, E: DenseCode> Default for TypedFastStateMachineBuilder<S, E> {
    /// Creates an empty definition with the default CAS policy.
    fn default() -> Self {
        Self::new()
    }
}
