/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Runtime errors returned by state transitions.

use std::error::Error;
use std::fmt::{
    self,
    Debug,
    Display,
    Formatter,
};

/// Error returned when an event cannot be applied to the current state.
///
/// `S` is the state type and `E` is the event type.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StateMachineError<S, E> {
    /// The current state is not registered in the state machine.
    UnknownState {
        /// The unregistered current state.
        state: S,
    },
    /// There is no transition for the current state and event pair.
    UnknownTransition {
        /// The current source state.
        source: S,
        /// The event that was triggered.
        event: E,
    },
    /// CAS retry limits were exhausted before a transition could be installed.
    CasConflict {
        /// Number of attempts executed by the CAS executor.
        attempts: u32,
    },
}

/// Result returned by event-triggering state machine operations.
///
/// `S` is the state type and `E` is the event type.
pub type StateMachineResult<S, E> = Result<S, StateMachineError<S, E>>;

impl<S, E> Display for StateMachineError<S, E>
where
    S: Debug,
    E: Debug,
{
    /// Formats the transition failure with enough context for diagnostics.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownState { state } => {
                write!(formatter, "unknown state: {state:?}")
            }
            Self::UnknownTransition { source, event } => {
                write!(formatter, "unknown transition: {source:?} --{event:?}--> ?")
            }
            Self::CasConflict { attempts } => {
                write!(
                    formatter,
                    "CAS transition failed after {attempts} attempt(s)"
                )
            }
        }
    }
}

impl<S, E> Error for StateMachineError<S, E>
where
    S: Debug,
    E: Debug,
{
}
