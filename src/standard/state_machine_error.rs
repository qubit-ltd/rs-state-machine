// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime errors returned by state transitions.

use std::fmt::Debug;

use qubit_cas::CasErrorKind;
use thiserror::Error;

/// Error returned when an event cannot be applied to the current state.
///
/// # Type Parameters
/// - `S`: State type recorded in the failed transition.
/// - `E`: Event type recorded in the failed transition.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::StateMachineError;
///
/// let error: StateMachineError<u8, u8> =
///     StateMachineError::UnknownState { state: 9 };
/// assert_eq!(error.to_string(), "unknown state: 9");
/// ```
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
    /// CAS execution terminated before a transition could be installed.
    #[error("CAS transition failed ({kind:?}) after {attempts} attempt(s)")]
    CasFailure {
        /// Terminal CAS error kind.
        kind: CasErrorKind,
        /// Number of attempts executed by the CAS executor.
        attempts: u32,
    },
}

/// Result returned by event-triggering state machine operations.
///
/// # Type Parameters
/// - `S`: State returned on success and recorded in runtime errors.
/// - `E`: Event recorded in unknown-transition errors.
pub type StateMachineResult<S, E> = Result<S, StateMachineError<S, E>>;
