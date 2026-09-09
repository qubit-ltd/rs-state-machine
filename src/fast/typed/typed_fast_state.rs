// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! An independently owned, typed atomic state cell.
use std::marker::PhantomData;

use qubit_fast_cas::FastCasState;

use super::DenseCode;
use super::dense_code::decode;

/// A compact state cell created by a typed machine.
///
/// `S` is the finite state type. The cell permits public reads and transitions
/// through machines with that same state type, even if their rules differ.
/// It has no machine identity, raw access, setter, default, or clone operation.
#[derive(Debug)]
pub struct TypedFastState<S: DenseCode> {
    /// Atomic code accessible only to the typed implementation.
    pub(super) raw: FastCasState,
    /// State type retained without additional storage.
    pub(super) marker: PhantomData<fn() -> S>,
}
impl<S: DenseCode> TypedFastState<S> {
    /// Loads the current state with acquire ordering.
    ///
    /// # Returns
    /// The typed state observed at the atomic load.
    ///
    /// # Panics
    /// Panics if an internal writer violated the validated state-code
    /// invariant.
    #[inline(always)]
    pub fn load(&self) -> S {
        decode(self.raw.load()).expect("typed state code must belong to the validated codebook")
    }
}
