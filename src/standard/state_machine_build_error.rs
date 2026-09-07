// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validation errors returned when building a state machine.

use std::fmt::Debug;

use thiserror::Error;

/// Error returned when state machine rules are internally inconsistent.
///
/// # Type Parameters
/// - `S`: State type recorded in the invalid rule.
/// - `E`: Event type recorded in the invalid transition.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Error)]
pub enum StateMachineBuildError<S, E>
where
    S: Debug,
    E: Debug,
{
    /// No initial state was configured.
    #[error("initial state is not configured")]
    InitialStateNotConfigured,
    /// An initial state was configured but not registered as a state.
    #[error("initial state is not registered: {state:?}")]
    InitialStateNotRegistered {
        /// The unregistered initial state.
        state: S,
    },
    /// A terminal state was configured but not registered as a state.
    #[error("terminal state is not registered: {state:?}")]
    TerminalStateNotRegistered {
        /// The unregistered terminal state.
        state: S,
    },
    /// A terminal state has an outgoing transition.
    #[error("terminal state has an outgoing transition: {state:?} --{event:?}--> {target:?}")]
    TerminalStateHasOutgoingTransition {
        /// Terminal source state.
        state: S,
        /// Event that would leave the terminal state.
        event: E,
        /// Target state of the forbidden transition.
        target: S,
    },
    /// A transition source was not registered as a state.
    #[error("transition source is not registered: {source_state:?} --{event:?}--> {target:?}")]
    TransitionSourceNotRegistered {
        /// Source state of the invalid transition.
        source_state: S,
        /// Event of the invalid transition.
        event: E,
        /// Target state of the invalid transition.
        target: S,
    },
    /// A transition target was not registered as a state.
    #[error("transition target is not registered: {source_state:?} --{event:?}--> {target:?}")]
    TransitionTargetNotRegistered {
        /// Source state of the invalid transition.
        source_state: S,
        /// Event of the invalid transition.
        event: E,
        /// Target state of the invalid transition.
        target: S,
    },
    /// Two transitions use the same `(source, event)` with different targets.
    #[error(
        "duplicate transition target: {source_state:?} --{event:?}--> \
         {existing_target:?} conflicts with {new_target:?}"
    )]
    DuplicateTransition {
        /// Source state shared by both transitions.
        source_state: S,
        /// Event shared by both transitions.
        event: E,
        /// Target registered first.
        existing_target: S,
        /// Conflicting target registered later.
        new_target: S,
    },
}
