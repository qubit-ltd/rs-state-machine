// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for fast state machine builder validation.

use qubit_fast_cas::FastCasPolicy;
use qubit_state_machine::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY;
use qubit_state_machine::FastStateMachine;
use qubit_state_machine::FastStateMachineBuildError;
use qubit_state_machine::FastStateMachineBuilder;

const QUEUED: u64 = 0;
const RUNNING: u64 = 1;
const SUCCEEDED: u64 = 2;
const FAILED: u64 = 3;

const START: u64 = 0;
const COMPLETE: u64 = 1;
const FAIL: u64 = 2;

fn create_valid_builder() -> FastStateMachineBuilder {
    FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .terminal_states(&[SUCCEEDED, FAILED])
        .transition(QUEUED, START, RUNNING)
        .transition(RUNNING, COMPLETE, SUCCEEDED)
        .transition(RUNNING, FAIL, FAILED)
}

#[test]
fn test_builder_build_accepts_valid_definition() {
    let machine = create_valid_builder()
        .build()
        .expect("valid fast definition should build");

    assert_eq!(machine.state_count(), 4);
    assert_eq!(machine.event_count(), 3);
    assert_eq!(machine.transition_count(), 3);
    assert!(machine.contains_state(QUEUED));
    assert!(!machine.contains_state(4));
    assert_eq!(machine.initial_state(), QUEUED);
    assert_eq!(machine.terminal_states().collect::<Vec<_>>(), vec![SUCCEEDED, FAILED]);
    assert_eq!(machine.transitions().collect::<Vec<_>>().len(), 3);
    assert!(machine.is_initial_state(QUEUED));
    assert!(machine.is_terminal_state(SUCCEEDED));
    assert!(machine.is_terminal_state(FAILED));
    assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
    assert_eq!(machine.transition_target(RUNNING, COMPLETE), Some(SUCCEEDED));
    assert_eq!(machine.transition_target(RUNNING, FAIL), Some(FAILED));
    assert_eq!(machine.create_state().load(), QUEUED);
}

#[test]
fn test_builder_accepts_u64_state_and_event_codes() {
    let state_count = 2_u64;
    let event_count = 1_u64;
    let source = 0_u64;
    let event = 0_u64;
    let target = 1_u64;

    let machine = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count)
        .initial_state(source)
        .terminal_state(target)
        .transition(source, event, target)
        .build()
        .expect("u64-coded state machine should build");

    assert_eq!(machine.state_count(), state_count);
    assert_eq!(machine.event_count(), event_count);
    assert_eq!(machine.transition_target(source, event), Some(target));
}

#[test]
fn test_builder_cas_policy_has_default_and_can_be_overridden() {
    assert_eq!(
        FastStateMachine::builder()
            .state_count(1)
            .event_count(1)
            .initial_state(QUEUED)
            .transition(QUEUED, START, QUEUED)
            .build()
            .expect("single-state machine should build with default policy")
            .transition_target(QUEUED, START),
        Some(QUEUED),
    );
    let custom_machine = FastStateMachine::builder()
        .state_count(1)
        .event_count(1)
        .initial_state(QUEUED)
        .cas_policy(FastCasPolicy::spin(8))
        .transition(QUEUED, START, QUEUED)
        .build()
        .expect("single-state machine should build with custom policy");
    assert_eq!(custom_machine.transition_target(QUEUED, START), Some(QUEUED));
}

#[test]
fn test_builder_supports_unique_initial_state_and_terminal_state() {
    let machine = FastStateMachine::builder()
        .state_count(3)
        .event_count(2)
        .initial_state(RUNNING)
        .terminal_state(SUCCEEDED)
        .transition(QUEUED, START, RUNNING)
        .transition(RUNNING, COMPLETE, SUCCEEDED)
        .build()
        .expect("builder should accept multi-state initial setup and final state");

    assert!(machine.is_initial_state(RUNNING));
    assert!(!machine.is_initial_state(SUCCEEDED));
    assert!(machine.is_terminal_state(SUCCEEDED));
    assert!(!machine.is_terminal_state(RUNNING));
}

#[test]
fn test_builder_default_constructor_still_available() {
    let machine = FastStateMachineBuilder::default()
        .state_count(1)
        .event_count(1)
        .initial_state(QUEUED)
        .transition(QUEUED, START, QUEUED)
        .build()
        .expect("default builder should be usable");

    assert_eq!(machine.transition_target(QUEUED, START), Some(QUEUED));
}

#[test]
fn test_builder_rejects_missing_state_count() {
    let builder = FastStateMachine::builder()
        .event_count(1)
        .initial_state(QUEUED)
        .transition(QUEUED, START, QUEUED);

    let error = builder
        .build()
        .expect_err("state_count is required to build a fast state machine");

    assert_eq!(error, FastStateMachineBuildError::StateCountNotConfigured);
}

#[test]
fn test_builder_rejects_missing_event_count() {
    let builder = FastStateMachine::builder()
        .state_count(1)
        .initial_state(QUEUED)
        .transition(QUEUED, START, QUEUED);

    let error = builder
        .build()
        .expect_err("event_count is required to build a fast state machine");

    assert_eq!(error, FastStateMachineBuildError::EventCountNotConfigured);
}

#[test]
fn test_builder_rejects_zero_state_count() {
    let builder = FastStateMachine::builder().state_count(0).event_count(1);

    let error = builder.build().expect_err("state_count must be positive");

    assert_eq!(error, FastStateMachineBuildError::InvalidStateCount { count: 0 });
}

#[test]
fn test_builder_rejects_zero_event_count() {
    let builder = FastStateMachine::builder().state_count(1).event_count(0);

    let error = builder.build().expect_err("event_count must be positive");

    assert_eq!(error, FastStateMachineBuildError::InvalidEventCount { count: 0 });
}

#[test]
fn test_builder_rejects_transition_table_overflow() {
    let error = FastStateMachine::builder()
        .state_count(u64::MAX)
        .event_count(2)
        .initial_state(0)
        .build()
        .expect_err("transition table size must fit u64");

    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionTableOverflow {
            state_count: u64::MAX,
            event_count: 2,
        }
    );
}

#[test]
fn test_builder_rejects_unallocatable_transition_table() {
    let error = FastStateMachine::builder()
        .state_count(u64::MAX)
        .event_count(1)
        .initial_state(0)
        .max_table_cells(u64::MAX)
        .build()
        .expect_err("unallocatable transition table must be rejected");

    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionTableCapacityExceeded {
            state_count: u64::MAX,
            event_count: 1,
        }
    );
}

#[test]
fn test_builder_validates_configuration_before_allocating_transition_table() {
    let invalid_initial_state = FastStateMachine::builder()
        .state_count(u64::MAX)
        .event_count(1)
        .initial_state(u64::MAX)
        .build()
        .expect_err("invalid initial state must be reported before allocation");
    assert_eq!(
        invalid_initial_state,
        FastStateMachineBuildError::InitialStateOutOfRange {
            state: u64::MAX,
            state_count: u64::MAX,
        }
    );

    let conflicting_transition = FastStateMachine::builder()
        .state_count(u64::MAX)
        .event_count(1)
        .initial_state(0)
        .transition(0, 0, 0)
        .transition(0, 0, 1)
        .build()
        .expect_err("conflicting transition must be reported before allocation");
    assert_eq!(
        conflicting_transition,
        FastStateMachineBuildError::DuplicateTransition {
            source_state: 0,
            event: 0,
            existing_target: 0,
            new_target: 1,
        }
    );
}

#[test]
fn test_builder_rejects_invalid_initial_state() {
    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(1)
        .initial_state(4)
        .transition(QUEUED, START, RUNNING)
        .build()
        .expect_err("initial state must be within range");

    assert_eq!(
        error,
        FastStateMachineBuildError::InitialStateOutOfRange {
            state: 4,
            state_count: 4,
        }
    );
}

#[test]
fn test_builder_rejects_invalid_terminal_state() {
    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(1)
        .initial_state(0)
        .terminal_state(4)
        .build()
        .expect_err("final state must be within range");

    assert_eq!(
        error,
        FastStateMachineBuildError::TerminalStateOutOfRange {
            state: 4,
            state_count: 4,
        }
    );
}

#[test]
fn test_builder_rejects_invalid_transition_elements() {
    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .transition(4, START, RUNNING)
        .build()
        .expect_err("transition source must be within state count");
    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionSourceOutOfRange {
            source_state: 4,
            state_count: 4,
        }
    );

    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .transition(QUEUED, 3, RUNNING)
        .build()
        .expect_err("transition event must be within range");
    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionEventOutOfRange {
            event: 3,
            event_count: 3,
        }
    );
    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .transition(QUEUED, START, 4)
        .build()
        .expect_err("transition target must be within range");
    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionTargetOutOfRange {
            target: 4,
            state_count: 4,
        }
    );
}

#[test]
fn test_builder_rejects_duplicate_transitions_with_different_targets() {
    let error = FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .transition(QUEUED, START, RUNNING)
        .transition(QUEUED, START, FAILED)
        .build()
        .expect_err("duplicated transition should be rejected");

    assert_eq!(
        error,
        FastStateMachineBuildError::DuplicateTransition {
            source_state: QUEUED,
            event: START,
            existing_target: RUNNING,
            new_target: FAILED,
        }
    );
}

#[test]
fn test_builder_accepts_exact_duplicate_transition() {
    let machine = FastStateMachine::builder()
        .state_count(2)
        .event_count(1)
        .initial_state(QUEUED)
        .transition(QUEUED, START, RUNNING)
        .transition(QUEUED, START, RUNNING)
        .build()
        .expect("repeating an identical transition should be idempotent");

    assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
}

#[test]
fn test_builder_default_policy_constant_is_reexported_and_reasonable() {
    let policy = FAST_STATE_MACHINE_DEFAULT_CAS_POLICY;
    assert_eq!(policy, FastCasPolicy::spin(16));
}

#[test]
fn test_builder_rejects_table_above_explicit_limit_before_allocation() {
    let error = FastStateMachine::builder()
        .state_count(2)
        .event_count(2)
        .initial_state(0)
        .max_table_cells(3)
        .build()
        .expect_err("four cells exceed a three-cell limit");
    assert_eq!(
        error,
        FastStateMachineBuildError::TransitionTableLimitExceeded {
            state_count: 2,
            event_count: 2,
            cells: 4,
            limit: 3,
        }
    );
}

#[test]
fn test_builder_accepts_table_at_explicit_limit() {
    let machine = FastStateMachine::builder()
        .state_count(2)
        .event_count(2)
        .initial_state(0)
        .max_table_cells(4)
        .build()
        .expect("four cells fit exactly");
    assert_eq!(machine.state_count(), 2);
}
