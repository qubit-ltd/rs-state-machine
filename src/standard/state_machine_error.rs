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

use std::fmt::Debug;

use thiserror::Error;

/// Error returned when an event cannot be applied to the current state.
///
/// `S` is the state type and `E` is the event type.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Error)]
pub enum StateMachineError<S, E>
where
    S: Debug,
    E: Debug,
{
    /// The current state is not registered in the state machine.
    #[error("unknown state: {state:?}")]
    UnknownState {
        /// The unregistered current state.
        state: S,
    },
    /// There is no transition for the current state and event pair.
    #[error("unknown transition: {source_state:?} --{event:?}--> ?")]
    UnknownTransition {
        /// The current source state.
        source_state: S,
        /// The event that was triggered.
        event: E,
    },
    /// CAS retry limits were exhausted before a transition could be installed.
    #[error("CAS transition failed after {attempts} attempt(s)")]
    CasConflict {
        /// Number of attempts executed by the CAS executor.
        attempts: u32,
    },
}

/// Result returned by event-triggering state machine operations.
///
/// `S` is the state type and `E` is the event type.
pub type StateMachineResult<S, E> = Result<S, StateMachineError<S, E>>;
