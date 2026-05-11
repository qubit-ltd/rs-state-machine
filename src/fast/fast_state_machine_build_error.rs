/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

//! Validation errors returned while building a fast state machine.

use thiserror::Error;

/// Error returned when fast state machine configuration is invalid.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Error)]
pub enum FastStateMachineBuildError {
    /// State count was not configured.
    #[error("state count is not configured")]
    StateCountNotConfigured,

    /// Event count was not configured.
    #[error("event count is not configured")]
    EventCountNotConfigured,

    /// State count must be greater than zero.
    #[error("state count must be positive: {count}")]
    InvalidStateCount {
        /// Requested state count.
        count: usize,
    },

    /// Event count must be greater than zero.
    #[error("event count must be positive: {count}")]
    InvalidEventCount {
        /// Requested event count.
        count: usize,
    },

    /// Transition transition-table size overflowed `usize`.
    #[error("transition table overflowed usize: {state_count} * {event_count}")]
    TransitionTableOverflow {
        /// The number of states.
        state_count: usize,

        /// The number of events.
        event_count: usize,
    },

    /// An initial state code exceeds the configured state count.
    #[error("initial state is out of range: {state} >= {state_count}")]
    InitialStateOutOfRange {
        /// The invalid initial state.
        state: usize,
        /// Configured state count.
        state_count: usize,
    },

    /// A final state code exceeds the configured state count.
    #[error("final state is out of range: {state} >= {state_count}")]
    FinalStateOutOfRange {
        /// The invalid final state.
        state: usize,
        /// Configured state count.
        state_count: usize,
    },

    /// A transition source code exceeds the configured state count.
    #[error("transition source is out of range: {source_state} >= {state_count}")]
    TransitionSourceOutOfRange {
        /// The invalid source state.
        source_state: usize,
        /// Configured state count.
        state_count: usize,
    },

    /// A transition event code exceeds the configured event count.
    #[error("transition event is out of range: {event} >= {event_count}")]
    TransitionEventOutOfRange {
        /// The invalid event code.
        event: usize,
        /// Configured event count.
        event_count: usize,
    },

    /// A transition target code exceeds the configured state count.
    #[error("transition target is out of range: {target} >= {state_count}")]
    TransitionTargetOutOfRange {
        /// The invalid target state.
        target: usize,
        /// Configured state count.
        state_count: usize,
    },

    /// Same `(source, event)` maps to two different targets.
    #[error(
        "duplicate transition: {source_state} --{event}--> {existing_target} conflicts with {new_target}"
    )]
    DuplicateTransition {
        /// Source state.
        source_state: usize,
        /// Event code.
        event: usize,
        /// Existing target.
        existing_target: usize,
        /// Conflicting target.
        new_target: usize,
    },
}
