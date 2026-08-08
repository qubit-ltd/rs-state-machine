// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

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
        count: u64,
    },

    /// Event count must be greater than zero.
    #[error("event count must be positive: {count}")]
    InvalidEventCount {
        /// Requested event count.
        count: u64,
    },

    /// Transition-table size overflowed `u64`.
    #[error("transition table size overflowed u64: {state_count} * {event_count}")]
    TransitionTableOverflow {
        /// The number of states.
        state_count: u64,

        /// The number of events.
        event_count: u64,
    },

    /// Transition-table storage cannot be represented or allocated.
    #[error("transition table capacity is unavailable: {state_count} * {event_count}")]
    TransitionTableCapacityExceeded {
        /// The number of states.
        state_count: u64,

        /// The number of events.
        event_count: u64,
    },

    /// An initial state code exceeds the configured state count.
    #[error("initial state is out of range: {state} >= {state_count}")]
    InitialStateOutOfRange {
        /// The invalid initial state.
        state: u64,
        /// Configured state count.
        state_count: u64,
    },

    /// A final state code exceeds the configured state count.
    #[error("final state is out of range: {state} >= {state_count}")]
    FinalStateOutOfRange {
        /// The invalid final state.
        state: u64,
        /// Configured state count.
        state_count: u64,
    },

    /// A transition source code exceeds the configured state count.
    #[error("transition source is out of range: {source_state} >= {state_count}")]
    TransitionSourceOutOfRange {
        /// The invalid source state.
        source_state: u64,
        /// Configured state count.
        state_count: u64,
    },

    /// A transition event code exceeds the configured event count.
    #[error("transition event is out of range: {event} >= {event_count}")]
    TransitionEventOutOfRange {
        /// The invalid event code.
        event: u64,
        /// Configured event count.
        event_count: u64,
    },

    /// A transition target code exceeds the configured state count.
    #[error("transition target is out of range: {target} >= {state_count}")]
    TransitionTargetOutOfRange {
        /// The invalid target state.
        target: u64,
        /// Configured state count.
        state_count: u64,
    },

    /// Same `(source, event)` maps to two different targets.
    #[error(
        "duplicate transition: {source_state} --{event}--> {existing_target} conflicts with {new_target}"
    )]
    DuplicateTransition {
        /// Source state.
        source_state: u64,
        /// Event code.
        event: u64,
        /// Existing target.
        existing_target: u64,
        /// Conflicting target.
        new_target: u64,
    },
}
