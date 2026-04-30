/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Immutable finite state machine rules and CAS-backed event triggering.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use qubit_atomic::AtomicRef;
use qubit_cas::{CasDecision, CasError, CasExecutor, CasSuccess};

use crate::{StateMachineBuilder, StateMachineError, StateMachineResult, Transition};

/// Immutable finite state machine rules.
///
/// `S` is the state type and `E` is the event type. Both should usually be
/// small enum-like types. The state machine itself is immutable and can be
/// shared across threads; mutable current state is kept in [`AtomicRef`] and
/// updated through [`qubit_cas::CasExecutor`].
#[derive(Debug, Clone)]
pub struct StateMachine<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    states: HashSet<S>,
    initial_states: HashSet<S>,
    final_states: HashSet<S>,
    transitions: HashSet<Transition<S, E>>,
    transition_map: HashMap<(S, E), S>,
    cas_executor: CasExecutor<S, StateMachineError<S, E>>,
}

impl<S, E> StateMachine<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    /// Creates a builder for immutable state machine rules.
    ///
    /// # Returns
    /// A new empty [`StateMachineBuilder`].
    pub fn builder() -> StateMachineBuilder<S, E> {
        StateMachineBuilder::new()
    }

    /// Creates a state machine from validated builder parts.
    ///
    /// # Parameters
    /// - `builder`: Builder containing validated states and terminal markers.
    /// - `transitions`: Validated transition set.
    /// - `transition_map`: Lookup table keyed by `(source, event)`.
    ///
    /// # Returns
    /// An immutable state machine.
    ///
    /// This constructor does not validate input. Rule validation belongs to
    /// [`StateMachineBuilder::build`].
    pub(crate) fn new(
        builder: StateMachineBuilder<S, E>,
        transitions: HashSet<Transition<S, E>>,
        transition_map: HashMap<(S, E), S>,
    ) -> Self {
        Self {
            states: builder.states,
            initial_states: builder.initial_states,
            final_states: builder.final_states,
            transitions,
            transition_map,
            cas_executor: CasExecutor::latency_first(),
        }
    }

    /// Returns all registered states.
    ///
    /// # Returns
    /// An immutable view of the registered state set.
    pub const fn states(&self) -> &HashSet<S> {
        &self.states
    }

    /// Returns all configured initial states.
    ///
    /// # Returns
    /// An immutable view of the initial state set.
    pub const fn initial_states(&self) -> &HashSet<S> {
        &self.initial_states
    }

    /// Returns all configured final states.
    ///
    /// # Returns
    /// An immutable view of the final state set.
    pub const fn final_states(&self) -> &HashSet<S> {
        &self.final_states
    }

    /// Returns all registered transitions.
    ///
    /// # Returns
    /// An immutable view of the transition set.
    pub const fn transitions(&self) -> &HashSet<Transition<S, E>> {
        &self.transitions
    }

    /// Tests whether a state is registered in this state machine.
    ///
    /// # Parameters
    /// - `state`: State to test.
    ///
    /// # Returns
    /// `true` if the state is registered.
    pub fn contains_state(&self, state: S) -> bool {
        self.states.contains(&state)
    }

    /// Tests whether a state is configured as an initial state.
    ///
    /// # Parameters
    /// - `state`: State to test.
    ///
    /// # Returns
    /// `true` if the state is an initial state.
    pub fn is_initial_state(&self, state: S) -> bool {
        self.initial_states.contains(&state)
    }

    /// Tests whether a state is configured as a final state.
    ///
    /// # Parameters
    /// - `state`: State to test.
    ///
    /// # Returns
    /// `true` if the state is a final state.
    pub fn is_final_state(&self, state: S) -> bool {
        self.final_states.contains(&state)
    }

    /// Looks up the target state for a source state and event.
    ///
    /// This method only queries rules; it does not modify any current-state
    /// storage.
    ///
    /// # Parameters
    /// - `source`: Source state.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// `Some(target)` if a transition exists, or `None` otherwise.
    pub fn transition_target(&self, source: S, event: E) -> Option<S> {
        self.transition_map.get(&(source, event)).copied()
    }

    /// Triggers an event and updates the provided atomic state reference.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// The new state after a successful transition.
    ///
    /// # Errors
    /// Returns [`StateMachineError::UnknownState`] when the current state is not
    /// registered. Returns [`StateMachineError::UnknownTransition`] when the
    /// current state is registered but has no transition for `event`.
    pub fn trigger(&self, state: &AtomicRef<S>, event: E) -> StateMachineResult<S, E> {
        let (_, new_state) = self.change_state(state, event)?;
        Ok(new_state)
    }

    /// Triggers an event, updates the atomic state, and invokes a success callback.
    ///
    /// The callback runs after the CAS update has succeeded.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    /// - `on_success`: Callback receiving `(old_state, new_state)`.
    ///
    /// # Returns
    /// The new state after a successful transition.
    ///
    /// # Errors
    /// Returns the same errors as [`StateMachine::trigger`]. The callback is not
    /// invoked when the transition fails.
    pub fn trigger_with<F>(
        &self,
        state: &AtomicRef<S>,
        event: E,
        on_success: F,
    ) -> StateMachineResult<S, E>
    where
        F: FnOnce(S, S),
    {
        let (old_state, new_state) = self.change_state(state, event)?;
        on_success(old_state, new_state);
        Ok(new_state)
    }

    /// Attempts to trigger an event without returning error details.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// `true` if the state changed successfully; `false` if the transition was
    /// invalid.
    pub fn try_trigger(&self, state: &AtomicRef<S>, event: E) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Attempts to trigger an event and invokes a callback only on success.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    /// - `on_success`: Callback receiving `(old_state, new_state)`.
    ///
    /// # Returns
    /// `true` if the state changed successfully; `false` if the transition was
    /// invalid. The callback is skipped when this method returns `false`.
    pub fn try_trigger_with<F>(&self, state: &AtomicRef<S>, event: E, on_success: F) -> bool
    where
        F: FnOnce(S, S),
    {
        self.trigger_with(state, event, on_success).is_ok()
    }

    /// Applies a transition through the CAS executor.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// The old and new state when the transition succeeds.
    ///
    /// # Errors
    /// Returns a runtime state machine error when no valid next state exists.
    fn change_state(
        &self,
        state: &AtomicRef<S>,
        event: E,
    ) -> Result<(S, S), StateMachineError<S, E>> {
        let outcome = self.cas_executor.execute(state, |current_state: &S| {
            match self.next_state(*current_state, event) {
                Ok(new_state) => CasDecision::update(new_state, new_state),
                Err(error) => CasDecision::abort(error),
            }
        });
        match outcome.into_result() {
            Ok(success) => Ok(Self::state_change_from_success(success)),
            Err(error) => Err(Self::state_error_from_cas_error(error)),
        }
    }

    /// Resolves the next state for the current state and event.
    ///
    /// # Parameters
    /// - `current_state`: State currently stored by the atomic reference.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// The target state when a transition exists.
    ///
    /// # Errors
    /// Returns an unknown-state error before checking transitions if the current
    /// state is not registered. Returns an unknown-transition error if no rule
    /// exists for the `(current_state, event)` pair.
    fn next_state(&self, current_state: S, event: E) -> Result<S, StateMachineError<S, E>> {
        if !self.contains_state(current_state) {
            return Err(StateMachineError::UnknownState {
                state: current_state,
            });
        }
        self.transition_target(current_state, event)
            .ok_or(StateMachineError::UnknownTransition {
                source: current_state,
                event,
            })
    }

    /// Extracts old and new states from a successful CAS transition.
    ///
    /// # Parameters
    /// - `success`: Successful CAS result returned by the executor.
    ///
    /// # Returns
    /// The old state and current state after CAS completion.
    fn state_change_from_success(success: CasSuccess<S, S>) -> (S, S) {
        match success {
            CasSuccess::Updated {
                previous, current, ..
            } => (*previous, *current),
            CasSuccess::Finished { current, .. } => (*current, *current),
        }
    }

    /// Maps terminal CAS failures into state machine errors.
    ///
    /// # Parameters
    /// - `error`: Terminal CAS error returned by the executor.
    ///
    /// # Returns
    /// The business state machine error when the operation aborted, or a CAS
    /// conflict error when retry limits were exhausted by compare-and-swap
    /// conflicts.
    fn state_error_from_cas_error(
        error: CasError<S, StateMachineError<S, E>>,
    ) -> StateMachineError<S, E> {
        match error.error() {
            Some(error) => *error,
            None => StateMachineError::CasConflict {
                attempts: error.attempts(),
            },
        }
    }
}
