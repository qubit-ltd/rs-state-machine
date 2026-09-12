// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Safe finite value encodings for typed dense state machines.
use std::fmt::Debug;

use super::TypedFastStateMachineBuildError;

/// A finite set of values encoded as contiguous integer codes.
///
/// `VALUES[i].code()` must equal `i`. Implementations must list every usable
/// value exactly once and return deterministic codes. Builders validate the
/// table; each typed input is also checked against its table entry, so an
/// omitted value cannot impersonate another value with the same code.
/// Decoding uses safe slice access; this trait never grants unsafe privileges.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::DenseCode;
///
/// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// assert_eq!(State::Ready.code(), 0);
/// ```
pub trait DenseCode: Copy + Eq + Debug + Send + Sync + 'static {
    /// Complete value set in ascending code order.
    const VALUES: &'static [Self];
    /// Returns this value's stable index in [`Self::VALUES`].
    ///
    /// The returned code must equal this value's index in `VALUES`; builders
    /// reject tables that do not satisfy this invariant.
    ///
    /// # Returns
    /// The stable dense code assigned to this value.
    #[must_use]
    fn code(self) -> u64;
}

/// Encodes `value` only when its declared table entry equals the value.
///
/// # Parameters
/// - `value`: Finite codebook member to encode.
///
/// # Returns
/// `Some(code)` for a valid encoding, or `None` for an omitted or invalid
/// value.
#[inline]
pub(super) fn checked_code<T: DenseCode>(value: T) -> Option<u64> {
    let code = value.code();
    let index = usize::try_from(code).ok()?;
    (T::VALUES.get(index).copied() == Some(value)).then_some(code)
}

/// Decodes `code` through the finite value table.
///
/// # Parameters
/// - `code`: Dense index to decode.
///
/// # Returns
/// The table value, or `None` if the code cannot index the table.
#[inline]
pub(super) fn decode<T: DenseCode>(code: u64) -> Option<T> {
    T::VALUES.get(usize::try_from(code).ok()?).copied()
}

/// Validates a nonempty, contiguous table before dense storage is allocated.
///
/// # Parameters
/// - `domain`: Diagnostic label, either `state` or `event`.
///
/// # Errors
/// Returns an empty-table error or the first mismatched index and code.
pub(super) fn validate_values<T: DenseCode>(domain: &'static str) -> Result<(), TypedFastStateMachineBuildError> {
    if T::VALUES.is_empty() {
        return Err(TypedFastStateMachineBuildError::EmptyCodebook { domain });
    }
    for (index, &value) in T::VALUES.iter().enumerate() {
        let code = value.code();
        if usize::try_from(code).ok() != Some(index) {
            return Err(TypedFastStateMachineBuildError::InvalidCodebook { domain, index, code });
        }
    }
    Ok(())
}
