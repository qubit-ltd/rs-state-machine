// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builder for immutable state machine rules.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use qubit_cas::CasExecutor;
use qubit_cas::CasStrategy;

use super::StateMachine;
use super::StateMachineBuildError;
use super::StateMachineError;
use crate::Transition;

/// Default number of CAS attempts for generic state-machine transitions.
///
/// The default is attempt-bounded but has no wall-clock budget or retry delay,
/// so ordinary transitions are not rejected because of scheduler timing.
pub const STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS: u32 = 16;

/// Builder used to define and validate finite state machine rules.
///
/// Configuration methods consume and return the builder so rule definitions
/// can be chained. The built [`StateMachine`] is immutable.
///
/// # Type Parameters
/// - `S`: Copyable, hashable state type.
/// - `E`: Copyable, hashable event type.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::StateMachineBuilder;
///
/// let machine = StateMachineBuilder::<u8, u8>::new()
///     .add_state(0)
///     .initial_state(0)
///     .transition(0, 0, 0)
///     .build()
///     .expect("the rules are valid");
/// assert_eq!(machine.transition_target(0, 0), Some(0));
/// ```
#[must_use = "a state machine builder must be configured and built"]
#[derive(Debug, Clone)]
pub struct StateMachineBuilder<S, E>
where
    S: Copy + Eq + Hash + Debug,
    E: Copy + Eq + Hash + Debug,
{
    /// Registered states accepted by the machine.
    pub(crate) states: HashSet<S>,
    /// The unique registered initial state.
    pub(crate) initial_state: Option<S>,
    /// Registered terminal states.
    pub(crate) terminal_states: HashSet<S>,
    /// Transition definitions in builder insertion order.
    pub(crate) transitions: Vec<Transition<S, E>>,
    /// CAS executor installed when the immutable machine is built.
    pub(crate) cas_executor: CasExecutor<S, StateMachineError<S, E>>,
}

impl<S, E> StateMachineBuilder<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    /// Creates an empty state machine builder.
    ///
    /// # Returns
    /// A builder with no states or transitions.
    #[inline]
    pub fn new() -> Self {
        Self {
            states: HashSet::new(),
            initial_state: None,
            terminal_states: HashSet::new(),
            transitions: Vec::new(),
            cas_executor: CasExecutor::builder()
                .max_attempts(STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS)
                .max_operation_elapsed(None)
                .max_total_elapsed(None)
                .flow_timeout(None)
                .attempt_timeout(None)
                .no_delay()
                .build()
                .expect("state machine default CAS configuration must be valid"),
        }
    }

    /// Adds a state to the state machine definition.
    ///
    /// # Parameters
    /// - `state`: State to register.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn add_state(mut self, state: S) -> Self {
        self.states.insert(state);
        self
    }

    /// Adds multiple states to the state machine definition.
    ///
    /// # Parameters
    /// - `states`: States to register.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn add_states(mut self, states: &[S]) -> Self {
        self.states.extend(states.iter().copied());
        self
    }

    /// Registers one initial state.
    ///
    /// The state must also be registered through [`add_state`](Self::add_state)
    /// or [`add_states`](Self::add_states) before [`build`](Self::build) is
    /// called.
    ///
    /// # Parameters
    /// - `state`: Initial state to add.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn initial_state(mut self, state: S) -> Self {
        self.initial_state = Some(state);
        self
    }

    /// Registers one terminal state.
    ///
    /// The state must also be registered through [`add_state`](Self::add_state)
    /// or [`add_states`](Self::add_states) before [`build`](Self::build) is
    /// called.
    ///
    /// # Parameters
    /// - `state`: Final state to add.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn terminal_state(mut self, state: S) -> Self {
        self.terminal_states.insert(state);
        self
    }

    /// Registers multiple terminal states.
    ///
    /// # Parameters
    /// - `states`: Final states to add.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn terminal_states(mut self, states: &[S]) -> Self {
        self.terminal_states.extend(states.iter().copied());
        self
    }

    /// Replaces the executor used for synchronous state transitions.
    ///
    /// # Parameters
    /// - `executor`: Validated retry limits, budgets and delays for
    ///   transitions.
    ///
    /// # Returns
    /// The updated builder. This replaces an earlier [`Self::cas_strategy`];
    /// a later strategy call replaces this executor in turn. Async hard
    /// timeouts are ignored by synchronous state transitions.
    #[inline]
    pub fn cas_executor(mut self, executor: CasExecutor<S, StateMachineError<S, E>>) -> Self {
        self.cas_executor = executor;
        self
    }

    /// Configures a built-in CAS execution strategy, replacing any injected
    /// executor.
    ///
    /// # Parameters
    /// - `strategy`: Built-in CAS retry strategy.
    ///
    /// # Returns
    /// The updated builder with an executor created from `strategy`.
    #[inline]
    pub fn cas_strategy(mut self, strategy: CasStrategy) -> Self {
        self.cas_executor = CasExecutor::with_strategy(strategy);
        self
    }

    /// Registers one transition by source state, event, and target state.
    ///
    /// Source and target states must be registered before
    /// [`build`](Self::build) is called. Adding the same transition more than
    /// once is allowed. Adding the same `(source, event)` with a different
    /// target is rejected during build.
    ///
    /// # Parameters
    /// - `source`: State before the event is applied.
    /// - `event`: Event that triggers the transition.
    /// - `target`: State after the transition succeeds.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn transition(self, source: S, event: E, target: S) -> Self {
        self.transition_value(Transition::new(source, event, target))
    }

    /// Registers one transition value.
    ///
    /// # Parameters
    /// - `transition`: Transition to add to the state machine definition.
    ///
    /// # Returns
    /// The updated builder.
    #[inline]
    pub fn transition_value(mut self, transition: Transition<S, E>) -> Self {
        self.transitions.push(transition);
        self
    }

    /// Builds an immutable state machine after validating the rule set.
    ///
    /// # Returns
    /// A validated immutable state machine.
    ///
    /// # Errors
    /// Returns a [`StateMachineBuildError`] when the initial or terminal state,
    /// transition source, or transition target is not registered, or when two
    /// transitions map the same `(source, event)` pair to different targets.
    pub fn build(self) -> Result<StateMachine<S, E>, StateMachineBuildError<S, E>> {
        let initial_state = self
            .initial_state
            .ok_or(StateMachineBuildError::InitialStateNotConfigured)?;
        if !self.states.contains(&initial_state) {
            return Err(StateMachineBuildError::InitialStateNotRegistered { state: initial_state });
        }
        self.validate_registered_states()?;

        let mut transition_map = HashMap::new();
        for transition in &self.transitions {
            let transition = *transition;
            self.validate_transition(transition)?;
            Self::insert_transition(transition, &mut transition_map)?;
        }

        Ok(StateMachine::new(self, transition_map))
    }

    /// Validates that configured final states are registered.
    ///
    /// # Returns
    /// `Ok(())` when all configured state sets refer to registered states.
    ///
    /// # Errors
    /// Returns the first unregistered final state encountered.
    fn validate_registered_states(&self) -> Result<(), StateMachineBuildError<S, E>> {
        for state in &self.terminal_states {
            if !self.states.contains(state) {
                return Err(StateMachineBuildError::TerminalStateNotRegistered { state: *state });
            }
        }
        Ok(())
    }

    /// Validates that a transition only references registered states.
    ///
    /// # Parameters
    /// - `transition`: Transition to validate.
    ///
    /// # Returns
    /// `Ok(())` when the transition source and target are registered.
    ///
    /// # Errors
    /// Returns the missing source or target as a build error.
    fn validate_transition(&self, transition: Transition<S, E>) -> Result<(), StateMachineBuildError<S, E>> {
        if !self.states.contains(&transition.source()) {
            return Err(StateMachineBuildError::TransitionSourceNotRegistered {
                source_state: transition.source(),
                event: transition.event(),
                target: transition.target(),
            });
        }
        if !self.states.contains(&transition.target()) {
            return Err(StateMachineBuildError::TransitionTargetNotRegistered {
                source_state: transition.source(),
                event: transition.event(),
                target: transition.target(),
            });
        }
        if self.terminal_states.contains(&transition.source()) {
            return Err(StateMachineBuildError::TerminalStateHasOutgoingTransition {
                state: transition.source(),
                event: transition.event(),
                target: transition.target(),
            });
        }
        Ok(())
    }

    /// Inserts a transition into the set and lookup table.
    ///
    /// # Parameters
    /// - `transition`: Transition to insert.
    /// - `transition_map`: Lookup table used for event triggering.
    ///
    /// # Returns
    /// `Ok(())` when the transition is inserted or is an exact duplicate.
    ///
    /// # Errors
    /// Returns a duplicate-transition error if the same source and event
    /// already point to a different target.
    fn insert_transition(
        transition: Transition<S, E>,
        transition_map: &mut HashMap<(S, E), S>,
    ) -> Result<(), StateMachineBuildError<S, E>> {
        let source = transition.source();
        let event = transition.event();
        let target = transition.target();
        if let Some(existing_target) = transition_map.get(&(source, event))
            && *existing_target != target
        {
            return Err(StateMachineBuildError::DuplicateTransition {
                source_state: source,
                event,
                existing_target: *existing_target,
                new_target: target,
            });
        }
        transition_map.insert((source, event), target);
        Ok(())
    }
}

impl<S, E> Default for StateMachineBuilder<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    /// Creates the same empty builder as [`Self::new`].
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
