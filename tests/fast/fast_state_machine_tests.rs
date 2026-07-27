// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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

use qubit_fast_cas::{
    FastCasPolicy,
    FastCasState,
};
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastStateMachine,
    FastStateMachineError,
};

const QUEUED: u64 = 0;
const RUNNING: u64 = 1;
const SUCCEEDED: u64 = 2;
const FAILED: u64 = 3;

const START: u64 = 0;
const COMPLETE: u64 = 1;
const FAIL: u64 = 2;
const TICK: u64 = 3;

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
fn test_trigger_preserves_high_u64_unknown_state() {
    let machine = create_machine();
    let high_state = 1_u64 << 32;
    let state = FastCasState::new(high_state);

    let error = machine
        .trigger(&state, START)
        .expect_err("high u64 state must not be truncated to a valid code");

    assert_eq!(
        error,
        FastStateMachineError::UnknownState { state: high_state }
    );
    assert_eq!(state.load(), high_state);
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
fn test_trigger_with_accepts_callback_that_consumes_capture() {
    let machine = create_machine();
    let state = FastCasState::new(QUEUED);
    let captured = String::from("consume once");

    let next = machine
        .trigger_with(&state, START, move |old_state, new_state| {
            assert_eq!((old_state, new_state), (QUEUED, RUNNING));
            drop(captured);
        })
        .expect("FnOnce callback should be accepted");

    assert_eq!(next, RUNNING);
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

    assert_eq!(machine.transition_target(u64::MAX, START), None);
    assert_eq!(machine.transition_target(QUEUED, u64::MAX), None);
}

#[test]
fn test_machine_handles_competing_alternating_transitions() {
    const THREAD_COUNT: usize = 8;
    const TRANSITIONS_PER_THREAD: usize = 128;
    const MAX_OUTER_ATTEMPTS: usize = 10_000;

    let machine = Arc::new(
        FastStateMachine::builder()
            .state_count(2)
            .event_count(1)
            .initial_state(0)
            .cas_policy(FastCasPolicy::once())
            .transition(0, 0, 1)
            .transition(1, 0, 0)
            .build()
            .expect("alternating state machine should build"),
    );
    let state = Arc::new(FastCasState::new(0));
    let zero_targets = Arc::new(AtomicUsize::new(0));
    let one_targets = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let mut handles = Vec::new();

    for _ in 0..THREAD_COUNT {
        let machine = Arc::clone(&machine);
        let state = Arc::clone(&state);
        let zero_targets = Arc::clone(&zero_targets);
        let one_targets = Arc::clone(&one_targets);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..TRANSITIONS_PER_THREAD {
                let mut transitioned = false;
                for _ in 0..MAX_OUTER_ATTEMPTS {
                    match machine.trigger_with(&state, 0, |_, target| {
                        if target == 0 {
                            zero_targets.fetch_add(1, Ordering::SeqCst);
                        } else {
                            one_targets.fetch_add(1, Ordering::SeqCst);
                        }
                    }) {
                        Ok(_) => {
                            transitioned = true;
                            break;
                        }
                        Err(FastStateMachineError::CasConflict { .. }) => {
                            thread::yield_now()
                        }
                        Err(error) => panic!(
                            "alternating transition should be valid: {error}"
                        ),
                    }
                }
                assert!(
                    transitioned,
                    "alternating transition should succeed within retry budget"
                );
            }
        }));
    }

    for handle in handles {
        handle.join().expect("worker should join");
    }

    let total_transitions = THREAD_COUNT * TRANSITIONS_PER_THREAD;
    assert_eq!(state.load(), 0);
    assert_eq!(zero_targets.load(Ordering::SeqCst), total_transitions / 2);
    assert_eq!(one_targets.load(Ordering::SeqCst), total_transitions / 2);
}

#[test]
fn test_state_setters_and_queries() {
    let machine = create_machine();
    let states = machine.transitions();

    assert_eq!(states.len(), 16);
    assert_eq!(machine.initial_states(), &[true, false, false, false]);
    assert_eq!(machine.final_states(), &[false, false, true, true]);
    assert!(machine.is_initial_state(QUEUED));
    assert!(!machine.is_initial_state(RUNNING));
    assert!(machine.is_final_state(SUCCEEDED));
    assert!(!machine.is_final_state(RUNNING));
    assert!(!machine.is_initial_state(9));
    assert!(!machine.is_final_state(9));
}
