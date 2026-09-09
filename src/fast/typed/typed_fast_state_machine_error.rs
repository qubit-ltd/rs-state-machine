// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Runtime failures from typed event validation or the shared Fast engine.
use thiserror::Error;

use crate::FastStateMachineError;

/// Error returned when a typed event cannot commit a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TypedFastStateMachineError {
    /// The supplied event is absent from its declared finite table.
    #[error("event value with code {code} is not in its codebook")]
    InvalidEventCode {
        /// The invalid event's claimed code.
        code: u64,
    },
    /// The integer engine rejected the event or exhausted its CAS budget.
    #[error(transparent)]
    Raw(#[from] FastStateMachineError),
}
