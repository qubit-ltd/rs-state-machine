// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for event-driven state transitions.

use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

use qubit_atomic::AtomicRef;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineError;
use qubit_state_machine::StateMachineResult;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    New,
    Running,
    Paused,
    Done,
    Failed,
    Detached,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Pause,
    Resume,
    Finish,
    Fail,
    Tick,
}

fn create_job_machine() -> StateMachine<JobState, JobEvent> {
    StateMachine::builder()
        .add_states(&[
            JobState::New,
            JobState::Running,
            JobState::Paused,
            JobState::Done,
            JobState::Failed,
        ])
        .initial_state(JobState::New)
        .terminal_states(&[JobState::Done, JobState::Failed])
        .transition(JobState::New, JobEvent::Start, JobState::Running)
        .transition(JobState::Running, JobEvent::Pause, JobState::Paused)
        .transition(JobState::Paused, JobEvent::Resume, JobState::Running)
        .transition(JobState::Running, JobEvent::Finish, JobState::Done)
        .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .transition(JobState::Running, JobEvent::Tick, JobState::Running)
        .build()
        .expect("job state machine rules should be valid")
}

fn trigger_start(
    machine: &StateMachine<JobState, JobEvent>,
    state: &AtomicRef<JobState>,
) -> StateMachineResult<JobState, JobEvent> {
    machine.trigger(state, JobEvent::Start)
}

#[test]
fn test_trigger_updates_state_and_returns_new_state() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);

    let new_state = trigger_start(&machine, &state).expect("start event should transition to running");

    assert_eq!(new_state, JobState::Running);
    assert_eq!(*state.load(), JobState::Running);
}

#[test]
fn test_trigger_returns_error_and_keeps_state_for_invalid_transition() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);

    let error = machine
        .trigger(&state, JobEvent::Finish)
        .expect_err("finish is invalid before start");

    assert_eq!(
        error,
        StateMachineError::UnknownTransition {
            source_state: JobState::New,
            event: JobEvent::Finish,
        }
    );
    assert_eq!(*state.load(), JobState::New);
}

#[test]
fn test_trigger_returns_error_and_keeps_state_for_unknown_current_state() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::Detached);

    let error = machine
        .trigger(&state, JobEvent::Start)
        .expect_err("current state is not registered");

    assert_eq!(
        error,
        StateMachineError::UnknownState {
            state: JobState::Detached,
        }
    );
    assert_eq!(*state.load(), JobState::Detached);
}

#[test]
fn test_state_machine_error_display_describes_failure_context() {
    assert_eq!(
        StateMachineError::<JobState, JobEvent>::UnknownState {
            state: JobState::Detached,
        }
        .to_string(),
        "unknown state: Detached"
    );
    assert_eq!(
        StateMachineError::UnknownTransition {
            source_state: JobState::New,
            event: JobEvent::Finish,
        }
        .to_string(),
        "unknown transition: New --Finish--> ?"
    );
    assert_eq!(
        StateMachineError::<JobState, JobEvent>::CasConflict { attempts: 3 }.to_string(),
        "CAS transition failed after 3 attempt(s)"
    );
}

#[test]
fn test_trigger_with_invokes_callback_after_successful_transition() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);
    let observed = Mutex::new(Vec::new());

    let new_state = machine
        .trigger_with(&state, JobEvent::Start, |old_state, new_state| {
            observed
                .lock()
                .expect("callback log should lock")
                .push((old_state, new_state, *state.load()));
        })
        .expect("start event should succeed");

    assert_eq!(new_state, JobState::Running);
    assert_eq!(
        observed.lock().expect("callback log should lock").as_slice(),
        &[(JobState::New, JobState::Running, JobState::Running)]
    );
}

#[test]
fn test_trigger_with_accepts_callback_that_consumes_capture() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);
    let captured = String::from("consume once");

    let next = machine
        .trigger_with(&state, JobEvent::Start, move |old_state, new_state| {
            assert_eq!((old_state, new_state), (JobState::New, JobState::Running));
            drop(captured);
        })
        .expect("FnOnce callback should be accepted");

    assert_eq!(next, JobState::Running);
}

#[test]
fn test_try_trigger_returns_true_and_updates_state_on_success() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);

    assert!(machine.try_trigger(&state, JobEvent::Start));

    assert_eq!(*state.load(), JobState::Running);
}

#[test]
fn test_try_trigger_returns_false_and_keeps_state_on_failure() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);

    assert!(!machine.try_trigger(&state, JobEvent::Finish));

    assert_eq!(*state.load(), JobState::New);
}

#[test]
fn test_try_trigger_with_skips_callback_on_failure() {
    let machine = create_job_machine();
    let state = AtomicRef::from_value(JobState::New);
    let callback_count = AtomicUsize::new(0);

    let triggered = machine.try_trigger_with(&state, JobEvent::Finish, |_, _| {
        callback_count.fetch_add(1, Ordering::SeqCst);
    });

    assert!(!triggered);
    assert_eq!(callback_count.load(Ordering::SeqCst), 0);
}

#[test]
fn test_trigger_handles_competing_alternating_transitions() {
    const THREAD_COUNT: usize = 8;
    const TRANSITIONS_PER_THREAD: usize = 64;
    const MAX_OUTER_ATTEMPTS: usize = 10_000;

    let machine = Arc::new(
        StateMachine::builder()
            .add_states(&[JobState::New, JobState::Running])
            .initial_state(JobState::New)
            .transition(JobState::New, JobEvent::Tick, JobState::Running)
            .transition(JobState::Running, JobEvent::Tick, JobState::New)
            .build()
            .expect("alternating state machine should build"),
    );
    let state = Arc::new(AtomicRef::from_value(JobState::New));
    let new_targets = Arc::new(AtomicUsize::new(0));
    let running_targets = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(THREAD_COUNT));
    let mut handles = Vec::new();

    for _ in 0..THREAD_COUNT {
        let machine = Arc::clone(&machine);
        let state = Arc::clone(&state);
        let new_targets = Arc::clone(&new_targets);
        let running_targets = Arc::clone(&running_targets);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            for _ in 0..TRANSITIONS_PER_THREAD {
                let mut transitioned = false;
                for _ in 0..MAX_OUTER_ATTEMPTS {
                    match machine.trigger_with(&state, JobEvent::Tick, |_, target| match target {
                        JobState::New => {
                            new_targets.fetch_add(1, Ordering::SeqCst);
                        }
                        JobState::Running => {
                            running_targets.fetch_add(1, Ordering::SeqCst);
                        }
                        _ => panic!("alternating transition produced an unexpected target"),
                    }) {
                        Ok(_) => {
                            transitioned = true;
                            break;
                        }
                        Err(StateMachineError::CasConflict { .. }) => thread::yield_now(),
                        Err(error) => panic!("alternating transition should be valid: {error}"),
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
        handle.join().expect("worker should complete");
    }

    let total_transitions = THREAD_COUNT * TRANSITIONS_PER_THREAD;
    assert_eq!(*state.load(), JobState::New);
    assert_eq!(new_targets.load(Ordering::SeqCst), total_transitions / 2);
    assert_eq!(running_targets.load(Ordering::SeqCst), total_transitions / 2);
}
