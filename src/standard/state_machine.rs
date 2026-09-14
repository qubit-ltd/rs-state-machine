// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Immutable finite state machine rules and CAS-backed event triggering.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use qubit_atomic::AtomicRef;
use qubit_cas::CasDecision;
use qubit_cas::CasError;
use qubit_cas::CasErrorKind;
use qubit_cas::CasExecutor;
use qubit_cas::CasSuccess;

use super::StateMachineBuilder;
use super::StateMachineError;
use super::StateMachineResult;
use crate::Transition;

/// Immutable finite state machine rules.
///
/// `S` is the state type and `E` is the event type. Both should usually be
/// small enum-like types. The state machine itself is immutable and can be
/// shared across threads; mutable current state is kept in [`AtomicRef`] and
/// updated through [`qubit_cas::CasExecutor`].
///
/// # Type Parameters
/// - `S`: Copyable, hashable state type stored in [`AtomicRef`].
/// - `E`: Copyable, hashable event type used to select transitions.
///
/// # Examples
///
/// Define the valid states and events, build an immutable transition table, and
/// keep each object's current state in an [`AtomicRef`].
///
/// ```
/// use qubit_atomic::AtomicRef;
/// use qubit_state_machine::StateMachine;
///
/// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
/// enum JobState {
///     Queued,
///     Running,
///     Succeeded,
///     Failed,
/// }
///
/// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
/// enum JobEvent {
///     Start,
///     Complete,
///     Fail,
/// }
///
/// fn create_job_machine() -> StateMachine<JobState, JobEvent> {
///     StateMachine::builder()
///         .add_states(&[
///             JobState::Queued,
///             JobState::Running,
///             JobState::Succeeded,
///             JobState::Failed,
///         ])
///         .initial_state(JobState::Queued)
///         .terminal_states(&[JobState::Succeeded, JobState::Failed])
///         .transition(JobState::Queued, JobEvent::Start, JobState::Running)
///         .transition(JobState::Running, JobEvent::Complete, JobState::Succeeded)
///         .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
///         .build()
///         .expect("job state machine should be valid")
/// }
///
/// let machine = create_job_machine();
/// let state = AtomicRef::from_value(JobState::Queued);
///
/// assert_eq!(machine.trigger(&state, JobEvent::Start).unwrap(), JobState::Running);
/// assert_eq!(*state.load(), JobState::Running);
///
/// assert!(machine.try_trigger(&state, JobEvent::Complete));
/// assert_eq!(*state.load(), JobState::Succeeded);
/// ```
#[must_use = "a state machine contains the configured transition rules"]
#[derive(Debug, Clone)]
pub struct StateMachine<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    /// Registered states accepted by the machine.
    states: HashSet<S>,
    /// The unique registered initial state.
    initial_state: S,
    /// Registered terminal states.
    terminal_states: HashSet<S>,
    /// Constant-time transition lookup keyed by `(source, event)`.
    transition_map: HashMap<(S, E), S>,
    /// CAS executor used to update external current-state references.
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
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_state_machine::StateMachine;
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    /// }
    ///
    /// let machine = StateMachine::<State, Event>::builder()
    ///     .add_state(State::New)
    ///     .initial_state(State::New)
    ///     .build()
    ///     .expect("single-state machine should build");
    /// assert!(machine.contains_state(State::New));
    /// ```
    #[inline]
    pub fn builder() -> StateMachineBuilder<S, E> {
        StateMachineBuilder::new()
    }

    /// Creates a state machine from validated builder parts.
    ///
    /// # Parameters
    /// - `builder`: Builder containing validated states and terminal markers.
    /// - `transition_map`: Lookup table keyed by `(source, event)`.
    ///
    /// # Returns
    /// An immutable state machine.
    ///
    /// This constructor does not validate input. Rule validation belongs to
    /// [`StateMachineBuilder::build`].
    #[inline]
    pub(crate) fn new(builder: StateMachineBuilder<S, E>, transition_map: HashMap<(S, E), S>) -> Self {
        Self {
            states: builder.states,
            initial_state: builder.initial_state.expect("builder validates initial state"),
            terminal_states: builder.terminal_states,
            transition_map,
            cas_executor: builder.cas_executor,
        }
    }

    /// Returns the executor containing the actual synchronous CAS
    /// configuration.
    ///
    /// # Returns
    /// A reference to the executor used by runtime transitions.
    #[must_use]
    #[inline]
    pub const fn cas_executor(&self) -> &CasExecutor<S, StateMachineError<S, E>> {
        &self.cas_executor
    }

    /// Analyzes reachability and paths to explicit terminal states.
    ///
    /// # Returns
    /// A report listing registered states unreachable from the initial state
    /// and states unable to reach an explicit terminal state.
    #[must_use]
    pub fn diagnose_graph(&self) -> crate::GraphDiagnostics<S> {
        let states: Vec<S> = self.states.iter().copied().collect();
        let indices: HashMap<S, usize> = states
            .iter()
            .copied()
            .enumerate()
            .map(|(index, state)| (state, index))
            .collect();
        let report = crate::diagnostics::analyze_graph(
            states.len(),
            indices[&self.initial_state],
            self.terminal_states.iter().map(|state| indices[state]),
            self.transition_map
                .iter()
                .map(|((source, _), target)| (indices[source], indices[target])),
        );
        crate::GraphDiagnostics {
            unreachable_states: report
                .unreachable_states
                .into_iter()
                .map(|index| states[index])
                .collect(),
            states_without_terminal_path: report
                .states_without_terminal_path
                .into_iter()
                .map(|index| states[index])
                .collect(),
        }
    }

    /// Returns all registered states.
    ///
    /// # Returns
    /// An immutable view of the registered state set.
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Running }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Start }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Running])
    /// #     .initial_state(State::New)
    /// #     .transition(State::New, Event::Start, State::Running)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert!(machine.states().contains(&State::New));
    /// assert_eq!(machine.states().len(), 2);
    /// ```
    #[must_use]
    #[inline]
    pub const fn states(&self) -> &HashSet<S> {
        &self.states
    }

    /// Returns the configured initial state.
    ///
    /// # Returns
    /// The unique configured initial state.
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Running }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Start }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Running])
    /// #     .initial_state(State::New)
    /// #     .transition(State::New, Event::Start, State::Running)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert_eq!(machine.initial_state(), State::New);
    /// ```
    #[must_use]
    #[inline]
    pub const fn initial_state(&self) -> S {
        self.initial_state
    }

    /// Returns all configured terminal states.
    ///
    /// # Returns
    /// An immutable view of the final state set.
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Done }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Finish }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Done])
    /// #     .initial_state(State::New)
    /// #     .terminal_state(State::Done)
    /// #     .transition(State::New, Event::Finish, State::Done)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert!(machine.terminal_states().contains(&State::Done));
    /// ```
    #[must_use]
    #[inline]
    pub const fn terminal_states(&self) -> &HashSet<S> {
        &self.terminal_states
    }

    /// Returns all registered transitions.
    ///
    /// # Returns
    /// An iterator of configured transition values.
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_state_machine::{StateMachine, Transition};
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    ///     Running,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    /// }
    ///
    /// let machine = StateMachine::builder()
    ///     .add_states(&[State::New, State::Running])
    ///     .initial_state(State::New)
    ///     .transition(State::New, Event::Start, State::Running)
    ///     .build()
    ///     .expect("rules should build");
    ///
    /// assert!(machine.transitions().any(|transition| {
    ///     transition == Transition::new(State::New, Event::Start, State::Running)
    /// }));
    /// ```
    #[must_use = "iterate over the configured transitions"]
    #[inline]
    pub fn transitions(&self) -> impl Iterator<Item = Transition<S, E>> + '_ {
        self.transition_map
            .iter()
            .map(|(&(source, event), &target)| Transition::new(source, event, target))
    }

    /// Tests whether a state is registered in this state machine.
    ///
    /// # Parameters
    /// - `state`: State to test.
    ///
    /// # Returns
    /// `true` if the state is registered.
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Running, Detached }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Start }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Running])
    /// #     .initial_state(State::New)
    /// #     .transition(State::New, Event::Start, State::Running)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert!(machine.contains_state(State::Running));
    /// assert!(!machine.contains_state(State::Detached));
    /// ```
    #[must_use]
    #[inline]
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Running }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Start }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Running])
    /// #     .initial_state(State::New)
    /// #     .transition(State::New, Event::Start, State::Running)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert!(machine.is_initial_state(State::New));
    /// assert!(!machine.is_initial_state(State::Running));
    /// ```
    #[must_use]
    #[inline]
    pub fn is_initial_state(&self, state: S) -> bool {
        self.initial_state == state
    }

    /// Tests whether a state is configured as a terminal state.
    ///
    /// # Parameters
    /// - `state`: State to test.
    ///
    /// # Returns
    /// `true` if the state is a final state.
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { Running, Done }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Finish }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::Running, State::Done])
    /// #     .initial_state(State::Running)
    /// #     .terminal_state(State::Done)
    /// #     .transition(State::Running, Event::Finish, State::Done)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert!(machine.is_terminal_state(State::Done));
    /// assert!(!machine.is_terminal_state(State::Running));
    /// ```
    #[must_use]
    #[inline]
    pub fn is_terminal_state(&self, state: S) -> bool {
        self.terminal_states.contains(&state)
    }

    /// Returns the number of registered states.
    ///
    /// # Returns
    /// The number of unique states in the immutable rule table.
    #[must_use]
    #[inline]
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    /// Returns the number of unique transitions.
    ///
    /// # Returns
    /// The number of unique `(source, event)` transition pairs.
    #[must_use]
    #[inline]
    pub fn transition_count(&self) -> usize {
        self.transition_map.len()
    }

    /// Creates an independent current-state cell initialized to the machine's
    /// initial state.
    ///
    /// # Returns
    /// A new atomic reference initialized to the configured initial state.
    #[must_use = "use the newly initialized state cell"]
    #[inline]
    pub fn create_state(&self) -> AtomicRef<S> {
        AtomicRef::from_value(self.initial_state)
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
    ///
    /// # Examples
    ///
    /// ```
    /// # use qubit_state_machine::StateMachine;
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum State { New, Running }
    /// # #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// # enum Event { Start, Finish }
    /// # let machine = StateMachine::builder()
    /// #     .add_states(&[State::New, State::Running])
    /// #     .initial_state(State::New)
    /// #     .transition(State::New, Event::Start, State::Running)
    /// #     .build()
    /// #     .expect("rules should build");
    /// assert_eq!(
    ///     machine.transition_target(State::New, Event::Start),
    ///     Some(State::Running),
    /// );
    /// assert_eq!(machine.transition_target(State::New, Event::Finish), None);
    /// ```
    #[must_use]
    #[inline]
    pub fn transition_target(&self, source: S, event: E) -> Option<S> {
        self.transition_map.get(&(source, event)).copied()
    }

    /// Triggers an event and updates the provided atomic state reference.
    ///
    /// External cells are not bound to a machine. Conflict retries resolve
    /// the event against each newly observed state; only state installation
    /// commits atomically, not associated payload or result publication.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    ///
    /// # Returns
    /// The new state after a successful transition.
    ///
    /// # Errors
    /// Returns [`StateMachineError::UnknownState`] when the current state is
    /// not registered. Returns [`StateMachineError::UnknownTransition`]
    /// when the current state is registered but has no transition for
    /// `event`. Returns [`StateMachineError::CasFailure`] when
    /// execution reaches a terminal limit of the configured executor policy.
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_atomic::AtomicRef;
    /// use qubit_state_machine::StateMachine;
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    ///     Running,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    /// }
    ///
    /// let machine = StateMachine::builder()
    ///     .add_states(&[State::New, State::Running])
    ///     .initial_state(State::New)
    ///     .transition(State::New, Event::Start, State::Running)
    ///     .build()
    ///     .expect("rules should build");
    /// let state = AtomicRef::from_value(State::New);
    ///
    /// assert_eq!(machine.trigger(&state, Event::Start).unwrap(), State::Running);
    /// assert_eq!(*state.load(), State::Running);
    /// ```
    pub fn trigger(&self, state: &AtomicRef<S>, event: E) -> StateMachineResult<S, E> {
        let (_, new_state) = self.change_state(state, event)?;
        Ok(new_state)
    }

    /// Triggers an event, updates the atomic state, and invokes a success
    /// callback.
    ///
    /// The callback runs once after the CAS update has succeeded. It may
    /// reenter the machine; concurrent callbacks need not follow commit order.
    /// It may observe a later state, while its arguments and the return value
    /// describe this commit. Payload publication is coordinated by the caller.
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
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
    /// Returns the same errors as [`StateMachine::trigger`]. The callback is
    /// not invoked when the transition fails.
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_atomic::AtomicRef;
    /// use qubit_state_machine::StateMachine;
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    ///     Running,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    /// }
    ///
    /// let machine = StateMachine::builder()
    ///     .add_states(&[State::New, State::Running])
    ///     .initial_state(State::New)
    ///     .transition(State::New, Event::Start, State::Running)
    ///     .build()
    ///     .expect("rules should build");
    /// let state = AtomicRef::from_value(State::New);
    /// let mut observed = None;
    ///
    /// let next = machine
    ///     .trigger_with(&state, Event::Start, |old_state, new_state| {
    ///         observed = Some((old_state, new_state));
    ///     })
    ///     .expect("start should be valid");
    ///
    /// assert_eq!(next, State::Running);
    /// assert_eq!(observed, Some((State::New, State::Running)));
    /// ```
    #[inline]
    pub fn trigger_with<F>(&self, state: &AtomicRef<S>, event: E, on_success: F) -> StateMachineResult<S, E>
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
    /// `true` if the transition was committed; `false` if validation or CAS
    /// execution failed. A successful self-transition also returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_atomic::AtomicRef;
    /// use qubit_state_machine::StateMachine;
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    ///     Running,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    ///     Finish,
    /// }
    ///
    /// let machine = StateMachine::builder()
    ///     .add_states(&[State::New, State::Running])
    ///     .initial_state(State::New)
    ///     .transition(State::New, Event::Start, State::Running)
    ///     .build()
    ///     .expect("rules should build");
    /// let state = AtomicRef::from_value(State::New);
    ///
    /// assert!(!machine.try_trigger(&state, Event::Finish));
    /// assert_eq!(*state.load(), State::New);
    /// assert!(machine.try_trigger(&state, Event::Start));
    /// ```
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline]
    pub fn try_trigger(&self, state: &AtomicRef<S>, event: E) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Attempts to trigger an event and invokes a callback only on success.
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
    ///
    /// # Parameters
    /// - `state`: Current state atomic reference.
    /// - `event`: Event to apply.
    /// - `on_success`: Callback receiving `(old_state, new_state)`.
    ///
    /// # Returns
    /// `true` if the transition was committed; `false` if validation or CAS
    /// execution failed. The callback is skipped when this method returns
    /// `false`.
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    ///
    /// # Examples
    ///
    /// ```
    /// use qubit_atomic::AtomicRef;
    /// use qubit_state_machine::StateMachine;
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum State {
    ///     New,
    ///     Running,
    /// }
    ///
    /// #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
    /// enum Event {
    ///     Start,
    ///     Finish,
    /// }
    ///
    /// let machine = StateMachine::builder()
    ///     .add_states(&[State::New, State::Running])
    ///     .initial_state(State::New)
    ///     .transition(State::New, Event::Start, State::Running)
    ///     .build()
    ///     .expect("rules should build");
    /// let state = AtomicRef::from_value(State::New);
    /// let mut callback_count = 0;
    ///
    /// assert!(!machine.try_trigger_with(&state, Event::Finish, |_, _| {
    ///     callback_count += 1;
    /// }));
    /// assert_eq!(callback_count, 0);
    ///
    /// assert!(machine.try_trigger_with(&state, Event::Start, |_, _| {
    ///     callback_count += 1;
    /// }));
    /// assert_eq!(callback_count, 1);
    /// ```
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline]
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
    fn change_state(&self, state: &AtomicRef<S>, event: E) -> Result<(S, S), StateMachineError<S, E>> {
        // Reject deterministic lookup failures before entering the retry
        // kernel.  Besides avoiding needless CAS work, this keeps invalid
        // events independent of the executor's terminal budget.
        let initial_state = *state.load();
        self.next_state(initial_state, event)?;
        let result = self.cas_executor.execute_result(state, |current_state: &S| {
            match self.next_state(*current_state, event) {
                Ok(new_state) => CasDecision::update(new_state, new_state),
                Err(error) => CasDecision::abort(error),
            }
        });
        match result {
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
    /// Returns an unknown-state error before checking transitions if the
    /// current state is not registered. Returns an unknown-transition error
    /// if no rule exists for the `(current_state, event)` pair.
    #[inline]
    fn next_state(&self, current_state: S, event: E) -> Result<S, StateMachineError<S, E>> {
        if !self.contains_state(current_state) {
            return Err(StateMachineError::UnknownState { state: current_state });
        }
        self.transition_target(current_state, event)
            .ok_or(StateMachineError::UnknownTransition {
                source_state: current_state,
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
    #[inline]
    fn state_change_from_success(success: CasSuccess<S, S>) -> (S, S) {
        match success {
            CasSuccess::Updated { previous, current, .. } => (*previous, *current),
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
    #[inline]
    fn state_error_from_cas_error(error: CasError<S, StateMachineError<S, E>>) -> StateMachineError<S, E> {
        Self::state_error_from_parts(error.kind(), error.attempts(), error.error().copied())
    }

    /// Projects a terminal CAS result while preserving an earlier business
    /// error.
    ///
    /// # Parameters
    /// - `kind`: Terminal CAS failure kind.
    /// - `attempts`: Number of CAS attempts performed.
    /// - `business`: Optional state-machine rejection produced by the decision
    ///   closure.
    ///
    /// # Returns
    /// The supplied business error when present; otherwise a `CasFailure` with
    /// the terminal kind and attempt count.
    fn state_error_from_parts(
        kind: CasErrorKind,
        attempts: u32,
        business: Option<StateMachineError<S, E>>,
    ) -> StateMachineError<S, E> {
        match business {
            Some(error) => error,
            None => StateMachineError::CasFailure { kind, attempts },
        }
    }
}

#[cfg(test)]
mod error_projection_tests {
    use std::sync::Arc;

    use qubit_atomic::AtomicRef;
    use qubit_cas::CasDecision;
    use qubit_cas::CasErrorKind;
    use qubit_cas::CasExecutor;

    use super::StateMachine;
    use crate::StateMachineError;

    #[test]
    fn test_budget_projection_preserves_kind_and_attempts() {
        for kind in [CasErrorKind::OperationBudgetExceeded, CasErrorKind::TotalBudgetExceeded] {
            assert_eq!(
                StateMachine::<u8, u8>::state_error_from_parts(kind, 7, None),
                StateMachineError::CasFailure { kind, attempts: 7 },
            );
        }
    }

    #[test]
    fn test_business_projection_preserves_original_error() {
        let business = StateMachineError::UnknownState { state: 7u8 };
        assert_eq!(
            StateMachine::<u8, u8>::state_error_from_parts(CasErrorKind::Abort, 1, Some(business),),
            business,
        );
    }

    #[test]
    fn test_real_executor_errors_reach_projection() {
        let executor = CasExecutor::<u8, StateMachineError<u8, u8>>::builder()
            .max_attempts(1)
            .no_delay()
            .build()
            .expect("valid executor");
        let state = AtomicRef::from_value(0u8);
        let business = StateMachineError::UnknownState { state: 7 };
        let aborted = executor
            .execute_result(&state, |_: &u8| {
                CasDecision::<u8, (), StateMachineError<u8, u8>>::abort(business)
            })
            .unwrap_err();
        assert_eq!(StateMachine::<u8, u8>::state_error_from_cas_error(aborted), business);

        let conflict = executor
            .execute_result(&state, |_: &u8| {
                state.store(Arc::new(1u8));
                CasDecision::update(2u8, ())
            })
            .unwrap_err();
        let kind = conflict.kind();
        assert_eq!(conflict.attempts(), 1);
        assert_eq!(
            StateMachine::<u8, u8>::state_error_from_cas_error(conflict),
            StateMachineError::CasFailure { kind, attempts: 1 },
        );
    }
}
