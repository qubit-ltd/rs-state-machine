/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache 2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for fast state machine runtime behavior.

use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};
use std::sync::{
    Arc,
    Barrier,
    Mutex,
};
use std::thread;

use qubit_cas::{
    FastCasError,
    FastCasState,
};
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastCasPolicy,
    FastStateMachine,
    FastStateMachineError,
    fast_state_machine_error_from_fast_cas_error,
};

const QUEUED: usize = 0;
const RUNNING: usize = 1;
const SUCCEEDED: usize = 2;
const FAILED: usize = 3;

const START: usize = 0;
const COMPLETE: usize = 1;
const FAIL: usize = 2;
const TICK: usize = 3;

fn create_machine() -> FastStateMachine {
    FastStateMachine::builder()
        .state_count(4)
        .event_count(4)
        .initial_state(QUEUED)
        .final_states(&[SUCCEEDED, FAILED])
        .transition(QUEUED, START, RUNNING)
        .transition(RUNNING, COMPLETE, SUCCEEDED)
        .transition(RUNNING, FAIL, FAILED)
        .transition(RUNNING, TICK, RUNNING)
        .build()
        .expect("fast machine should build")
}

#[test]
fn test_trigger_updates_fast_state_and_returns_next_state() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);

    let next = machine
        .trigger(&state, START)
        .expect("start transition should be valid");

    assert_eq!(next, RUNNING);
    assert_eq!(state.load(), RUNNING);
}

#[test]
fn test_trigger_returns_error_for_unknown_transition_and_keeps_state() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);

    let error = machine
        .trigger(&state, COMPLETE)
        .expect_err("queued state has no complete transition");

    assert_eq!(
        error,
        FastStateMachineError::UnknownTransition {
            source_state: QUEUED,
            event: COMPLETE,
        }
    );
    assert_eq!(state.load(), QUEUED);
}

#[test]
fn test_trigger_returns_error_for_unknown_state() {
    let machine = create_machine();
    let state = FastCasState::new(9);

    let error = machine
        .trigger(&state, START)
        .expect_err("unknown state should fail");

    assert_eq!(error, FastStateMachineError::UnknownState { state: 9 });
    assert_eq!(state.load(), 9);
}

#[test]
fn test_trigger_with_calls_callback_after_success() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);
    let callback_states = Arc::new(Mutex::new(Vec::new()));
    let callback_states_for_capture = Arc::clone(&callback_states);

    let next = machine
        .trigger_with(&state, START, |old_state, new_state| {
            callback_states_for_capture
                .lock()
                .expect("callback state should lock")
                .push((old_state, new_state));
        })
        .expect("start should succeed");

    assert_eq!(next, RUNNING);
    assert_eq!(
        callback_states
            .lock()
            .expect("callback state should lock")
            .as_slice(),
        &[(QUEUED, RUNNING)],
    );
    assert_eq!(state.load(), RUNNING);
}

#[test]
fn test_try_trigger_is_boolean_result_without_error() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);

    assert!(machine.try_trigger(&state, START));
    assert_eq!(state.load(), RUNNING);
    assert!(!machine.try_trigger(&state, START));
    assert_eq!(state.load(), RUNNING);
}

#[test]
fn test_try_trigger_with_calls_callback_only_on_success() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);
    let callback_count = AtomicUsize::new(0);

    let matched = machine.try_trigger_with(&state, COMPLETE, |_, _| {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });
    assert!(!matched);
    assert_eq!(callback_count.load(Ordering::SeqCst), 0);
    assert_eq!(state.load(), QUEUED);

    let matched = machine.try_trigger_with(&state, START, |_, _| {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });
    assert!(matched);
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    assert_eq!(state.load(), RUNNING);
}

#[test]
fn test_transition_target_is_constant_time_lookup_for_known_codes() {
    let machine = create_machine();

    assert_eq!(machine.transition_target(RUNNING, TICK), Some(RUNNING));
    assert_eq!(machine.transition_target(RUNNING, FAIL), Some(FAILED));
    assert_eq!(machine.transition_target(FAILED, START), None);
}

#[test]
fn test_cas_policy_is_readable_from_machine() {
    let default_machine = FastStateMachine::builder()
        .state_count(1)
        .event_count(1)
        .initial_state(QUEUED)
        .transition(QUEUED, START, QUEUED)
        .build()
        .expect("single-state machine should build");

    assert_eq!(
        default_machine.cas_policy(),
        FAST_STATE_MACHINE_DEFAULT_CAS_POLICY
    );

    let custom_policy = FastCasPolicy::spin(8);
    let custom_machine = FastStateMachine::builder()
        .state_count(1)
        .event_count(1)
        .initial_state(QUEUED)
        .cas_policy(custom_policy)
        .transition(QUEUED, START, QUEUED)
        .build()
        .expect("single-state machine should build with custom policy");

    assert_eq!(custom_machine.cas_policy(), custom_policy);
}

#[test]
fn test_transition_target_returns_none_for_out_of_range_input() {
    let machine = create_machine();

    assert_eq!(machine.transition_target(usize::MAX, START), None);
    assert_eq!(machine.transition_target(QUEUED, usize::MAX), None);
}

#[test]
fn test_fast_cas_conflict_maps_to_fast_state_machine_error() {
    let error = fast_state_machine_error_from_fast_cas_error(FastCasError::Conflict {
        current: RUNNING,
        attempts: 1,
    });

    assert_eq!(error, FastStateMachineError::CasConflict { attempts: 1 });
}

#[test]
fn test_machine_handles_concurrent_self_transitions() {
    let machine = Arc::new(create_machine());
    let state = Arc::new(FastCasState::new(RUNNING));
    let callback_count = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));
    let mut handles = Vec::new();

    for _ in 0..8 {
        let machine = Arc::clone(&machine);
        let state = Arc::clone(&state);
        let callback_count = Arc::clone(&callback_count);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            machine
                .trigger_with(&state, TICK, |_, _| {
                    callback_count.fetch_add(1, Ordering::SeqCst);
                })
                .expect("self transition should always remain valid");
        }));
    }

    for handle in handles {
        handle.join().expect("worker should join");
    }

    assert_eq!(state.load(), RUNNING);
    assert_eq!(callback_count.load(Ordering::SeqCst), 8);
}

#[test]
fn test_state_setters_and_queries() {
    let machine = create_machine();
    let states = machine.transitions();

    assert_eq!(states.len(), 16);
    assert!(machine.initial_states()[QUEUED]);
    assert!(!machine.initial_states()[RUNNING]);
    assert!(machine.final_states()[SUCCEEDED]);
    assert!(!machine.final_states()[RUNNING]);
    assert!(!machine.is_initial_state(9));
    assert!(!machine.is_final_state(9));
}
