/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Builder for immutable state machine rules.

use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use crate::{StateMachine, StateMachineBuildError, Transition};

/// Builder used to define and validate finite state machine rules.
///
/// `S` is the state type and `E` is the event type. The builder is mutable; the
/// built [`StateMachine`] is immutable.
#[derive(Debug, Clone)]
pub struct StateMachineBuilder<S, E>
where
    S: Copy + Eq + Hash + Debug,
    E: Copy + Eq + Hash + Debug,
{
    pub(crate) states: HashSet<S>,
    pub(crate) initial_states: HashSet<S>,
    pub(crate) final_states: HashSet<S>,
    pub(crate) transitions: Vec<Transition<S, E>>,
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
    pub fn new() -> Self {
        Self {
            states: HashSet::new(),
            initial_states: HashSet::new(),
            final_states: HashSet::new(),
            transitions: Vec::new(),
        }
    }

    /// Adds a state to the state machine definition.
    ///
    /// # Parameters
    /// - `state`: State to register.
    pub fn add_state(&mut self, state: S) {
        self.states.insert(state);
    }

    /// Adds multiple states to the state machine definition.
    ///
    /// # Parameters
    /// - `states`: States to register.
    pub fn add_states(&mut self, states: &[S]) {
        self.states.extend(states.iter().copied());
    }

    /// Marks a state as an initial state.
    ///
    /// The state must also be registered through [`add_state`](Self::add_state)
    /// or [`add_states`](Self::add_states) before [`build`](Self::build) is
    /// called.
    ///
    /// # Parameters
    /// - `state`: Initial state to add.
    pub fn set_initial_state(&mut self, state: S) {
        self.initial_states.insert(state);
    }

    /// Marks multiple states as initial states.
    ///
    /// # Parameters
    /// - `states`: Initial states to add.
    pub fn set_initial_states(&mut self, states: &[S]) {
        self.initial_states.extend(states.iter().copied());
    }

    /// Marks a state as a final state.
    ///
    /// The state must also be registered through [`add_state`](Self::add_state)
    /// or [`add_states`](Self::add_states) before [`build`](Self::build) is
    /// called.
    ///
    /// # Parameters
    /// - `state`: Final state to add.
    pub fn set_final_state(&mut self, state: S) {
        self.final_states.insert(state);
    }

    /// Marks multiple states as final states.
    ///
    /// # Parameters
    /// - `states`: Final states to add.
    pub fn set_final_states(&mut self, states: &[S]) {
        self.final_states.extend(states.iter().copied());
    }

    /// Adds a transition by source state, event, and target state.
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
    pub fn add_transition(&mut self, source: S, event: E, target: S) {
        self.add_transition_value(Transition::new(source, event, target));
    }

    /// Adds a transition value.
    ///
    /// # Parameters
    /// - `transition`: Transition to add to the state machine definition.
    pub fn add_transition_value(&mut self, transition: Transition<S, E>) {
        self.transitions.push(transition);
    }

    /// Builds an immutable state machine after validating the rule set.
    ///
    /// # Returns
    /// A validated immutable state machine.
    ///
    /// # Errors
    /// Returns a [`StateMachineBuildError`] when an initial state, final state,
    /// transition source, or transition target is not registered, or when two
    /// transitions map the same `(source, event)` pair to different targets.
    pub fn build(self) -> Result<StateMachine<S, E>, StateMachineBuildError<S, E>> {
        StateMachine::new(self)
    }
}

impl<S, E> Default for StateMachineBuilder<S, E>
where
    S: Copy + Eq + Hash + Debug + 'static,
    E: Copy + Eq + Hash + Debug + 'static,
{
    /// Creates an empty state machine builder.
    fn default() -> Self {
        Self::new()
    }
}
