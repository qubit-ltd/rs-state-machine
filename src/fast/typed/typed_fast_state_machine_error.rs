// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Runtime failures from typed event validation or the shared Fast engine.
use thiserror::Error;

use crate::FastStateMachineError;

/// Error returned when a typed event cannot commit a transition.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::TypedFastStateMachineError;
///
/// let error = TypedFastStateMachineError::InvalidEventCode { code: 9 };
/// assert!(!error.is_unknown_transition());
/// ```
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

impl TypedFastStateMachineError {
    /// Returns whether the supplied event has no configured transition.
    ///
    /// # Returns
    /// `true` only for a wrapped unknown-transition error.
    #[must_use]
    pub const fn is_unknown_transition(&self) -> bool {
        matches!(self, Self::Raw(FastStateMachineError::UnknownTransition { .. }))
    }

    /// Returns whether the CAS retry budget was exhausted.
    ///
    /// # Returns
    /// `true` only for a wrapped CAS-conflict error.
    #[must_use]
    pub const fn is_cas_conflict(&self) -> bool {
        matches!(self, Self::Raw(FastStateMachineError::CasConflict { .. }))
    }
}
