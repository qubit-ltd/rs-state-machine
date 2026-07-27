// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Immutable transition value.

/// A directed transition in a finite state machine.
///
/// In normal use, the state and event types are small enum-like values.
///
/// # Type Parameters
/// - `S`: State value stored as both the source and target.
/// - `E`: Event value that selects the transition.
#[must_use = "a transition describes a configured state change"]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Transition<S, E> {
    /// State required before the transition.
    source: S,
    /// Event that triggers the transition.
    event: E,
    /// State installed after the transition.
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
    #[inline(always)]
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
    #[inline(always)]
    pub const fn source(&self) -> S {
        self.source
    }

    /// Returns the event that triggers this transition.
    ///
    /// # Returns
    /// The event associated with this transition.
    #[inline(always)]
    pub const fn event(&self) -> E {
        self.event
    }

    /// Returns the target state of this transition.
    ///
    /// # Returns
    /// The state after this transition succeeds.
    #[inline(always)]
    pub const fn target(&self) -> S {
        self.target
    }
}
