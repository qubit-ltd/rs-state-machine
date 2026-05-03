/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Validation errors returned when building a state machine.

use std::error::Error;
use std::fmt::{
    self,
    Debug,
    Display,
    Formatter,
};

/// Error returned when state machine rules are internally inconsistent.
///
/// `S` is the state type and `E` is the event type.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StateMachineBuildError<S, E> {
    /// An initial state was configured but not registered as a state.
    InitialStateNotRegistered {
        /// The unregistered initial state.
        state: S,
    },
    /// A final state was configured but not registered as a state.
    FinalStateNotRegistered {
        /// The unregistered final state.
        state: S,
    },
    /// A transition source was not registered as a state.
    TransitionSourceNotRegistered {
        /// Source state of the invalid transition.
        source: S,
        /// Event of the invalid transition.
        event: E,
        /// Target state of the invalid transition.
        target: S,
    },
    /// A transition target was not registered as a state.
    TransitionTargetNotRegistered {
        /// Source state of the invalid transition.
        source: S,
        /// Event of the invalid transition.
        event: E,
        /// Target state of the invalid transition.
        target: S,
    },
    /// Two transitions use the same `(source, event)` with different targets.
    DuplicateTransition {
        /// Source state shared by both transitions.
        source: S,
        /// Event shared by both transitions.
        event: E,
        /// Target registered first.
        existing_target: S,
        /// Conflicting target registered later.
        new_target: S,
    },
}

impl<S, E> Display for StateMachineBuildError<S, E>
where
    S: Debug,
    E: Debug,
{
    /// Formats the validation error with the offending state or transition.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InitialStateNotRegistered { state } => {
                write!(formatter, "initial state is not registered: {state:?}")
            }
            Self::FinalStateNotRegistered { state } => {
                write!(formatter, "final state is not registered: {state:?}")
            }
            Self::TransitionSourceNotRegistered {
                source,
                event,
                target,
            } => write!(
                formatter,
                "transition source is not registered: {source:?} --{event:?}--> {target:?}"
            ),
            Self::TransitionTargetNotRegistered {
                source,
                event,
                target,
            } => write!(
                formatter,
                "transition target is not registered: {source:?} --{event:?}--> {target:?}"
            ),
            Self::DuplicateTransition {
                source,
                event,
                existing_target,
                new_target,
            } => write!(
                formatter,
                "duplicate transition target: {source:?} --{event:?}--> \
                 {existing_target:?} conflicts with {new_target:?}"
            ),
        }
    }
}

impl<S, E> Error for StateMachineBuildError<S, E>
where
    S: Debug,
    E: Debug,
{
}
