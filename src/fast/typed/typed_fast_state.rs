// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
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
///
/// # Type Parameters
/// - `S`: Finite state type implementing [`DenseCode`].
///
/// # Examples
///
/// ```
/// use qubit_state_machine::{DenseCode, TypedFastStateMachine};
///
/// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// #[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// enum Event { Tick }
/// impl DenseCode for Event {
///     const VALUES: &'static [Self] = &[Self::Tick];
///     fn code(self) -> u64 { 0 }
/// }
/// let machine = TypedFastStateMachine::<State, Event>::builder()
///     .initial_state(State::Ready).build().expect("valid rules");
/// assert_eq!(machine.create_state().load(), State::Ready);
/// ```
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
    #[must_use]
    #[inline(always)]
    pub fn load(&self) -> S {
        decode(self.raw.load()).expect("typed state code must belong to the validated codebook")
    }
}
