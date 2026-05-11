/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for fast state machine builder validation.

use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastCasPolicy,
    FastStateMachine,
    FastStateMachineBuildError,
    FastStateMachineBuilder,
};

const QUEUED: usize = 0;
const RUNNING: usize = 1;
const SUCCEEDED: usize = 2;
const FAILED: usize = 3;

const START: usize = 0;
const COMPLETE: usize = 1;
const FAIL: usize = 2;

fn create_valid_builder() -> FastStateMachineBuilder {
    FastStateMachine::builder()
        .state_count(4)
        .event_count(3)
        .initial_state(QUEUED)
        .final_states(&[SUCCEEDED, FAILED])
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
    assert!(machine.is_initial_state(QUEUED));
    assert!(machine.is_final_state(SUCCEEDED));
    assert!(machine.is_final_state(FAILED));
    assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
    assert_eq!(
        machine.transition_target(RUNNING, COMPLETE),
        Some(SUCCEEDED)
    );
    assert_eq!(machine.transition_target(RUNNING, FAIL), Some(FAILED));
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
    assert_eq!(
        custom_machine.transition_target(QUEUED, START),
        Some(QUEUED)
    );
}

#[test]
fn test_builder_supports_initial_states_and_final_state() {
    let machine = FastStateMachine::builder()
        .state_count(3)
        .event_count(2)
        .initial_states(&[QUEUED, RUNNING])
        .final_state(SUCCEEDED)
        .transition(QUEUED, START, RUNNING)
        .transition(RUNNING, COMPLETE, SUCCEEDED)
        .build()
        .expect("builder should accept multi-state initial setup and final state");

    assert!(machine.initial_states()[QUEUED]);
    assert!(machine.initial_states()[RUNNING]);
    assert!(!machine.initial_states()[SUCCEEDED]);
    assert!(machine.final_states()[SUCCEEDED]);
    assert!(!machine.final_states()[RUNNING]);
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

    assert_eq!(
        error,
        FastStateMachineBuildError::InvalidStateCount { count: 0 }
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
fn test_builder_default_policy_constant_is_reexported_and_reasonable() {
    let policy = FAST_STATE_MACHINE_DEFAULT_CAS_POLICY;
    assert_eq!(policy, FastCasPolicy::spin(16));
}
