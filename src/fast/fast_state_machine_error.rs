/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

//! Runtime errors returned by `FastStateMachine` transitions.

use thiserror::Error;

/// Error returned when applying an event through a [`crate::FastStateMachine`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Error)]
pub enum FastStateMachineError {
    /// The current state code is not configured in the state machine.
    #[error("unknown state: {state}")]
    UnknownState {
        /// The unregistered current state code.
        state: usize,
    },

    /// No transition is configured for the `(state, event)` pair.
    #[error("unknown transition: {source_state} --{event}--> ?")]
    UnknownTransition {
        /// The source state code.
        source_state: usize,

        /// The triggering event code.
        event: usize,
    },

    /// CAS conflicts were exhausted before the update could be installed.
    #[error("CAS transition failed after {attempts} attempt(s)")]
    CasConflict {
        /// The total number of CAS attempts executed.
        attempts: u32,
    },
}

/// Result returned by `FastStateMachine` runtime transition APIs.
pub type FastStateMachineResult = Result<usize, FastStateMachineError>;
