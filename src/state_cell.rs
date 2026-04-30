/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Thread-safe state storage for state machines.

use std::sync::Mutex;

/// Thread-safe storage for the current state.
///
/// `StateCell` is the Rust counterpart to the Java implementation's
/// `AtomicReference<S>` parameter. It uses a mutex because generic enum-like
/// states cannot be updated with a portable lock-free compare-and-swap API.
#[derive(Debug)]
pub struct StateCell<S> {
    value: Mutex<S>,
}

impl<S> StateCell<S>
where
    S: Copy,
{
    /// Creates a state cell with the given initial state.
    ///
    /// # Parameters
    /// - `state`: Initial state stored by the cell.
    ///
    /// # Returns
    /// A new [`StateCell`].
    pub const fn new(state: S) -> Self {
        Self {
            value: Mutex::new(state),
        }
    }

    /// Reads the current state.
    ///
    /// # Returns
    /// The state currently stored in the cell.
    ///
    /// # Panics
    /// Panics if the internal mutex has been poisoned by a previous panic while
    /// the state was being read or updated.
    pub fn get(&self) -> S {
        *self
            .value
            .lock()
            .expect("state cell mutex should not be poisoned while reading state")
    }

    /// Replaces the current state and returns the previous one.
    ///
    /// This method updates the state under a mutex, so concurrent readers and
    /// writers observe a single consistent value.
    ///
    /// # Parameters
    /// - `state`: New state to store.
    ///
    /// # Returns
    /// The state that was stored before replacement.
    ///
    /// # Panics
    /// Panics if the internal mutex has been poisoned by a previous panic while
    /// the state was being read or updated.
    pub fn replace(&self, state: S) -> S {
        let mut guard = self
            .value
            .lock()
            .expect("state cell mutex should not be poisoned while replacing state");
        let old_state = *guard;
        *guard = state;
        old_state
    }

    /// Applies a fallible state replacement while holding the mutex.
    ///
    /// # Parameters
    /// - `operation`: Function that maps the current state to the next state.
    ///
    /// # Returns
    /// The old and new state when `operation` succeeds, or the operation error
    /// with the stored state left unchanged.
    ///
    /// # Panics
    /// Panics if the internal mutex has been poisoned by a previous panic while
    /// the state was being read or updated.
    pub(crate) fn try_replace_with<F, T>(&self, operation: F) -> Result<(S, S), T>
    where
        F: FnOnce(S) -> Result<S, T>,
    {
        let mut guard = self
            .value
            .lock()
            .expect("state cell mutex should not be poisoned while updating state");
        let old_state = *guard;
        let new_state = operation(old_state)?;
        *guard = new_state;
        Ok((old_state, new_state))
    }
}
