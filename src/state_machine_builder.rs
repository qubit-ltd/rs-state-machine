/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Builder for immutable state machine rules.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

use crate::{StateMachine, StateMachineBuildError, Transition};

/// Builder used to define and validate finite state machine rules.
///
/// `S` is the state type and `E` is the event type. Configuration methods
/// consume and return the builder so rule definitions can be chained. The built
/// [`StateMachine`] is immutable.
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
    ///
    /// # Returns
    /// The updated builder.
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
    pub fn add_states(mut self, states: &[S]) -> Self {
        self.states.extend(states.iter().copied());
        self
    }

    /// Marks a state as an initial state.
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
    pub fn set_initial_state(mut self, state: S) -> Self {
        self.initial_states.insert(state);
        self
    }

    /// Marks multiple states as initial states.
    ///
    /// # Parameters
    /// - `states`: Initial states to add.
    ///
    /// # Returns
    /// The updated builder.
    pub fn set_initial_states(mut self, states: &[S]) -> Self {
        self.initial_states.extend(states.iter().copied());
        self
    }

    /// Marks a state as a final state.
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
    pub fn set_final_state(mut self, state: S) -> Self {
        self.final_states.insert(state);
        self
    }

    /// Marks multiple states as final states.
    ///
    /// # Parameters
    /// - `states`: Final states to add.
    ///
    /// # Returns
    /// The updated builder.
    pub fn set_final_states(mut self, states: &[S]) -> Self {
        self.final_states.extend(states.iter().copied());
        self
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
    ///
    /// # Returns
    /// The updated builder.
    pub fn add_transition(self, source: S, event: E, target: S) -> Self {
        self.add_transition_value(Transition::new(source, event, target))
    }

    /// Adds a transition value.
    ///
    /// # Parameters
    /// - `transition`: Transition to add to the state machine definition.
    ///
    /// # Returns
    /// The updated builder.
    pub fn add_transition_value(mut self, transition: Transition<S, E>) -> Self {
        self.transitions.push(transition);
        self
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
        self.validate_registered_states()?;

        let mut transition_set = HashSet::new();
        let mut transition_map = HashMap::new();
        for transition in &self.transitions {
            let transition = *transition;
            self.validate_transition(transition)?;
            Self::insert_transition(transition, &mut transition_set, &mut transition_map)?;
        }

        Ok(StateMachine::new(self, transition_set, transition_map))
    }

    /// Validates that initial and final states are registered.
    ///
    /// # Returns
    /// `Ok(())` when all configured state sets refer to registered states.
    ///
    /// # Errors
    /// Returns the first unregistered initial or final state encountered.
    fn validate_registered_states(&self) -> Result<(), StateMachineBuildError<S, E>> {
        for state in &self.initial_states {
            if !self.states.contains(state) {
                return Err(StateMachineBuildError::InitialStateNotRegistered { state: *state });
            }
        }
        for state in &self.final_states {
            if !self.states.contains(state) {
                return Err(StateMachineBuildError::FinalStateNotRegistered { state: *state });
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
    fn validate_transition(
        &self,
        transition: Transition<S, E>,
    ) -> Result<(), StateMachineBuildError<S, E>> {
        if !self.states.contains(&transition.source()) {
            return Err(StateMachineBuildError::TransitionSourceNotRegistered {
                source: transition.source(),
                event: transition.event(),
                target: transition.target(),
            });
        }
        if !self.states.contains(&transition.target()) {
            return Err(StateMachineBuildError::TransitionTargetNotRegistered {
                source: transition.source(),
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
    /// - `transition_set`: Set used for public transition inspection.
    /// - `transition_map`: Lookup table used for event triggering.
    ///
    /// # Returns
    /// `Ok(())` when the transition is inserted or is an exact duplicate.
    ///
    /// # Errors
    /// Returns a duplicate-transition error if the same source and event already
    /// point to a different target.
    fn insert_transition(
        transition: Transition<S, E>,
        transition_set: &mut HashSet<Transition<S, E>>,
        transition_map: &mut HashMap<(S, E), S>,
    ) -> Result<(), StateMachineBuildError<S, E>> {
        let source = transition.source();
        let event = transition.event();
        let target = transition.target();
        if let Some(existing_target) = transition_map.get(&(source, event))
            && *existing_target != target
        {
            return Err(StateMachineBuildError::DuplicateTransition {
                source,
                event,
                existing_target: *existing_target,
                new_target: target,
            });
        }
        transition_set.insert(transition);
        transition_map.insert((source, event), target);
        Ok(())
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
