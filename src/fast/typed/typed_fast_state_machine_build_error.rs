// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Validation failures for typed finite encodings and transition rules.
use thiserror::Error;

use crate::FastStateMachineBuildError;

/// Error returned when a typed machine's encoding or rules are invalid.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::TypedFastStateMachineBuildError;
///
/// let error = TypedFastStateMachineBuildError::EmptyCodebook { domain: "state" };
/// assert_eq!(error.to_string(), "state codebook is empty");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TypedFastStateMachineBuildError {
    /// The state or event value table is empty.
    #[error("{domain} codebook is empty")]
    EmptyCodebook {
        /// The table domain: `state` or `event`.
        domain: &'static str,
    },
    /// A table entry does not encode its own index.
    #[error("{domain} codebook index {index} has code {code}")]
    InvalidCodebook {
        /// The table domain: `state` or `event`.
        domain: &'static str,
        /// The invalid entry's index.
        index: usize,
        /// The code returned by that entry.
        code: u64,
    },
    /// A supplied value is absent from its declared table.
    #[error("{domain} value with code {code} is not in its codebook")]
    ValueNotInCodebook {
        /// The input domain: `state` or `event`.
        domain: &'static str,
        /// The code claimed by the supplied value.
        code: u64,
    },
    /// The shared integer builder rejected the transition definition.
    #[error(transparent)]
    Raw(
        /// The underlying integer-builder error.
        #[from]
        FastStateMachineBuildError,
    ),
}
