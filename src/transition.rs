// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Immutable transition value.

/// A directed transition in a finite state machine.
///
/// # Type Parameters
/// - `S`: State value used for the source and target.
/// - `E`: Event value that triggers the transition.
///
/// # Examples
///
/// ```
/// use qubit_state_machine::Transition;
///
/// let transition = Transition::new("queued", "start", "running");
/// assert_eq!(transition.source(), "queued");
/// assert_eq!(transition.event(), "start");
/// assert_eq!(transition.target(), "running");
/// ```
#[must_use = "a transition describes a configured state change"]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Transition<S, E> {
    /// State required before the transition.
    source: S,
    /// Event that selects the transition.
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
    /// - `source`: State required before the event.
    /// - `event`: Event that selects the transition.
    /// - `target`: State installed after the transition.
    ///
    /// # Returns
    /// A transition value containing the three supplied values.
    #[inline]
    pub const fn new(source: S, event: E, target: S) -> Self {
        Self { source, event, target }
    }

    /// Returns the source state.
    ///
    /// # Returns
    /// The state required before this transition can be applied.
    #[must_use]
    #[inline]
    pub const fn source(&self) -> S {
        self.source
    }

    /// Returns the triggering event.
    ///
    /// # Returns
    /// The event that selects this transition.
    #[must_use]
    #[inline]
    pub const fn event(&self) -> E {
        self.event
    }

    /// Returns the target state.
    ///
    /// # Returns
    /// The state installed after this transition succeeds.
    #[must_use]
    #[inline]
    pub const fn target(&self) -> S {
        self.target
    }
}
