/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Immutable transition value.

/// A directed transition in a finite state machine.
///
/// `S` is the state type and `E` is the event type. In normal use both are
/// small enum-like values that implement `Copy`, `Eq`, and `Hash`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Transition<S, E> {
    source: S,
    event: E,
    target: S,
}

impl<S, E> Transition<S, E>
where
    S: Copy,
    E: Copy,
{
    /// Creates a transition from `source` to `target` triggered by `event`.
    ///
    /// # Parameters
    /// - `source`: State before the event is applied.
    /// - `event`: Event that triggers this transition.
    /// - `target`: State after the transition succeeds.
    ///
    /// # Returns
    /// A new immutable transition value.
    pub const fn new(source: S, event: E, target: S) -> Self {
        Self {
            source,
            event,
            target,
        }
    }

    /// Returns the source state of this transition.
    ///
    /// # Returns
    /// The state that must be current before this transition can be applied.
    pub const fn source(&self) -> S {
        self.source
    }

    /// Returns the event that triggers this transition.
    ///
    /// # Returns
    /// The event associated with this transition.
    pub const fn event(&self) -> E {
        self.event
    }

    /// Returns the target state of this transition.
    ///
    /// # Returns
    /// The state after this transition succeeds.
    pub const fn target(&self) -> S {
        self.target
    }
}
