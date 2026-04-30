/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! CAS-backed current-state storage for state machines.

use qubit_atomic::AtomicRef;

/// CAS-compatible storage for the current state.
///
/// This alias keeps the state-machine domain name available while exposing the
/// same type and API as [`qubit_atomic::AtomicRef`].
pub type StateCell<S> = AtomicRef<S>;
