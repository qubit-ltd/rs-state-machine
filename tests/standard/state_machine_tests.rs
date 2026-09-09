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
use qubit_cas::CasErrorKind;
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
        StateMachineError::<JobState, JobEvent>::CasFailure {
            kind: CasErrorKind::RetryExhausted,
            attempts: 3,
        }
        .to_string(),
        "CAS transition failed (RetryExhausted) after 3 attempt(s)"
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
                        Err(StateMachineError::CasFailure { .. }) => thread::yield_now(),
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

#[cfg(feature = "standard")]
#[test]
fn test_standard_strategy_reports_actual_configuration() {
    use qubit_cas::CasExecutor;
    use qubit_cas::CasStrategy;
    use qubit_state_machine::STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS;
    let default = StateMachine::<u8, u8>::builder()
        .add_state(0)
        .initial_state(0)
        .build()
        .expect("machine definition is valid");
    assert_eq!(
        default
            .cas_executor()
            .retry_policy()
            .admission_limits()
            .max_attempts()
            .get(),
        STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS
    );
    let custom = StateMachine::<u8, u8>::builder()
        .add_state(0)
        .initial_state(0)
        .cas_strategy(CasStrategy::ReliabilityFirst)
        .build()
        .expect("machine definition is valid");
    assert_eq!(
        custom
            .cas_executor()
            .retry_policy()
            .admission_limits()
            .max_attempts()
            .get(),
        CasExecutor::<JobState, StateMachineError<JobState, JobEvent>>::reliability_first()
            .retry_policy()
            .admission_limits()
            .max_attempts()
            .get()
    );
}

mod runtime_contracts {
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::time::Duration;

    use qubit_state_machine::StateMachine;
    use qubit_state_machine::StateMachineBuildError;
    type Machine = StateMachine<u64, u64>;

    /// Builds the same three-state lifecycle for each backend.
    fn create_machine() -> Machine {
        Machine::builder()
            .add_states(&[0, 1, 2])
            .initial_state(0)
            .terminal_state(2)
            .transition(0, 0, 1)
            .transition(1, 1, 2)
            .build()
            .expect("lifecycle definition is valid")
    }

    #[test]
    fn test_initial_state_and_terminal_edges() {
        assert!(matches!(
            Machine::builder().add_states(&[0, 1, 2]).build(),
            Err(StateMachineBuildError::InitialStateNotConfigured)
        ));
        for target in [0, 1] {
            assert!(
                matches!(Machine::builder().add_states(&[0, 1, 2]).initial_state(0).terminal_state(0)
                .transition(0, 0, target).build(),
                Err(StateMachineBuildError::TerminalStateHasOutgoingTransition { state: 0, event: 0, target: actual }) if actual == target)
            );
        }
        let terminal = Machine::builder()
            .add_states(&[0, 1, 2])
            .initial_state(0)
            .terminal_state(0)
            .build()
            .expect("an initial terminal state may have no edges");
        assert!(terminal.is_terminal_state(terminal.initial_state()));
        let state = terminal.create_state();
        assert!(!terminal.try_trigger(&state, 0));
        let overwritten = Machine::builder()
            .add_states(&[0, 1, 2])
            .initial_state(0)
            .initial_state(1)
            .build()
            .expect("the last initial state wins");
        assert_eq!(overwritten.initial_state(), 1);
        let state = overwritten.create_state();
        assert_eq!(*state.load(), 1);
    }

    #[test]
    fn test_self_transition_commits_once_and_failure_skips_callback() {
        let machine = Machine::builder()
            .add_states(&[0, 1, 2])
            .initial_state(0)
            .transition(0, 0, 0)
            .transition(0, 0, 0)
            .build()
            .expect("duplicate self edge is valid");
        assert_eq!(machine.transition_count(), 1);
        let state = machine.create_state();
        let mut calls = Vec::new();
        assert!(machine.try_trigger_with(&state, 0, |old, new| calls.push((old, new))));
        assert!(!machine.try_trigger_with(&state, 2, |old, new| calls.push((old, new))));
        assert_eq!(calls, vec![(0, 0)]);
        assert_eq!(*state.load(), 0);
        assert!(matches!(
            Machine::builder()
                .add_states(&[0, 1, 2])
                .initial_state(0)
                .transition(0, 0, 1)
                .transition(0, 0, 2)
                .build(),
            Err(StateMachineBuildError::DuplicateTransition { .. })
        ));
    }

    #[test]
    fn test_created_states_are_independent() {
        let machine = create_machine();
        let first = machine.create_state();
        let state = machine.create_state();
        assert_eq!(machine.trigger(&first, 0).expect("start commits"), 1);
        assert_eq!(*state.load(), 0);
    }

    #[test]
    fn test_callback_reentry_returns_own_target() {
        let machine = create_machine();
        let state = machine.create_state();
        let result = machine
            .trigger_with(&state, 0, |old, new| {
                assert_eq!((old, new), (0, 1));
                assert_eq!(machine.trigger(&state, 1).expect("nested finish commits"), 2);
            })
            .expect("outer start commits");
        assert_eq!(result, 1);
        assert_eq!(*state.load(), 2);
    }

    #[test]
    fn test_callback_panic_preserves_commit() {
        let machine = create_machine();
        let state = machine.create_state();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _result = machine.trigger_with(&state, 0, |_, _| panic!("callback panicked"));
        }));
        assert!(outcome.is_err());
        assert_eq!(*state.load(), 1);
    }

    #[test]
    fn test_callbacks_can_observe_later_state_and_finish_out_of_order() {
        let machine = Arc::new(create_machine());
        let state = Arc::new(machine.create_state());
        let order = Arc::new(Mutex::new(Vec::new()));
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let thread_machine = Arc::clone(&machine);
        let thread_state = Arc::clone(&state);
        let thread_order = Arc::clone(&order);
        let worker = std::thread::spawn(move || {
            thread_machine
                .trigger_with(&thread_state, 0, |old, new| {
                    entered_tx.send(()).expect("receiver remains alive");
                    release_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("later callback releases earlier callback");
                    assert_eq!((old, new), (0, 1));
                    let state = &thread_state;
                    assert_eq!(*state.load(), 2);
                    thread_order.lock().expect("order lock is healthy").push(0);
                })
                .expect("first transition commits")
        });
        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("first callback entered");
        machine
            .trigger_with(&state, 1, |old, new| {
                assert_eq!((old, new), (1, 2));
                order.lock().expect("order lock is healthy").push(1);
            })
            .expect("second transition commits");
        release_tx.send(()).expect("earlier callback is waiting");
        assert_eq!(worker.join().expect("worker finishes"), 1);
        assert_eq!(*order.lock().expect("order lock is healthy"), vec![1, 0]);
    }
}
