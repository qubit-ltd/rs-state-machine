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

/// Error returned when a typed event cannot commit a transition.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::{DenseCode, TypedFastStateMachineError};
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum State { Ready }
/// impl DenseCode for State { const VALUES: &'static [Self] = &[Self::Ready]; fn code(self) -> u64 { 0 } }
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Event { Tick }
/// impl DenseCode for Event { const VALUES: &'static [Self] = &[Self::Tick]; fn code(self) -> u64 { 0 } }
///
/// let error = TypedFastStateMachineError::<State, Event>::InvalidEventCode {
///     event: Event::Tick, code: 9,
/// };
/// assert!(!error.is_unknown_transition());
/// ```
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TypedFastStateMachineError<S: super::DenseCode, E: super::DenseCode> {
    /// The supplied event is absent from its declared finite table.
    #[error("event {event:?} with code {code} is not in its codebook")]
    InvalidEventCode {
        /// The invalid event value.
        event: E,
        /// The invalid event's claimed code.
        code: u64,
    },
    /// No transition is configured for the typed pair.
    #[error("unknown transition: {source_state:?} --{event:?}--> ?")]
    UnknownTransition {
        /// The current typed state.
        source_state: S,
        /// The triggering event.
        event: E,
    },
    /// CAS retries were exhausted.
    #[error("CAS transition failed after {attempts} attempt(s)")]
    CasConflict {
        /// Number of exhausted CAS attempts.
        attempts: u32,
    },
    /// The stored state code is outside the typed codebook.
    #[error("state code {code} is not in its codebook")]
    InvalidStateCode {
        /// The invalid stored code.
        code: u64,
    },
}

impl<S: super::DenseCode, E: super::DenseCode> TypedFastStateMachineError<S, E> {
    /// Returns whether the supplied event has no configured transition.
    ///
    /// # Returns
    /// `true` only for a wrapped unknown-transition error.
    #[must_use]
    #[inline]
    pub const fn is_unknown_transition(&self) -> bool {
        matches!(self, Self::UnknownTransition { .. })
    }

    /// Returns whether the CAS retry budget was exhausted.
    ///
    /// # Returns
    /// `true` only for a wrapped CAS-conflict error.
    #[must_use]
    #[inline]
    pub const fn is_cas_conflict(&self) -> bool {
        matches!(self, Self::CasConflict { .. })
    }
}
