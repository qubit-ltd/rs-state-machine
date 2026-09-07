// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Immutable transition value.

/// A directed transition in a finite state machine.
#[must_use = "a transition describes a configured state change"]
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
    #[inline(always)]
    pub const fn new(source: S, event: E, target: S) -> Self {
        Self { source, event, target }
    }

    /// Returns the source state.
    #[must_use]
    #[inline(always)]
    pub const fn source(&self) -> S {
        self.source
    }

    /// Returns the triggering event.
    #[must_use]
    #[inline(always)]
    pub const fn event(&self) -> E {
        self.event
    }

    /// Returns the target state.
    #[must_use]
    #[inline(always)]
    pub const fn target(&self) -> S {
        self.target
    }
}
