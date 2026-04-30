/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Runtime errors returned by state transitions.

use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};

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
}

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
        }
    }
}

impl<S, E> Error for StateMachineError<S, E>
where
    S: Debug,
    E: Debug,
{
}
