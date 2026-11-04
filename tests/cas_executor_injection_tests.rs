// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Standard state machine configuration and post-commit callback contracts.
#![cfg(feature = "standard")]

use std::time::Duration;
use std::sync::Barrier;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::thread;

use qubit_atomic::AtomicRef;
use qubit_cas::CasErrorKind;
use qubit_cas::CasExecutor;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineError;

#[test]
fn test_initial_precheck_precedes_zero_elapsed_budgets() {
    for (operation, total, kind) in [
        (Some(Duration::ZERO), None, CasErrorKind::OperationBudgetExceeded),
        (None, Some(Duration::ZERO), CasErrorKind::TotalBudgetExceeded),
    ] {
        let executor = CasExecutor::<u8, StateMachineError<u8, u8>>::builder()
            .max_attempts(1)
            .max_operation_elapsed(operation)
            .max_total_elapsed(total)
            .flow_timeout(None)
            .attempt_timeout(None)
            .no_delay()
            .build()
            .expect("zero elapsed budget is valid");
        let machine = StateMachine::builder()
            .add_states(&[0_u8, 1])
            .initial_state(0)
            .transition(0, 0_u8, 1)
            .cas_executor(executor)
            .build()
            .expect("valid transition rules");
        for (current, event, expected) in [
            (9, 0, StateMachineError::UnknownState { state: 9 }),
            (0, 1, StateMachineError::UnknownTransition { source_state: 0, event: 1 }),
            (0, 0, StateMachineError::CasFailure { kind, attempts: 0 }),
        ] {
            let state = AtomicRef::from_value(current);
            assert_eq!(machine.trigger(&state, event), Err(expected));
            assert_eq!(*state.load(), current);
            let callback_state = AtomicRef::from_value(current);
            let mut callbacks = 0;
            assert_eq!(
                machine.trigger_with(&callback_state, event, |_, _| callbacks += 1),
                Err(expected),
            );
            assert_eq!(*callback_state.load(), current);
            assert_eq!(callbacks, 0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum State {
    New,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Event {
    Start,
}

#[test]
fn test_injected_executor_and_success_callback_work() {
    let executor = CasExecutor::<State, StateMachineError<State, Event>>::builder()
        .max_attempts(1)
        .no_delay()
        .build()
        .expect("valid executor");
    let machine = StateMachine::builder()
        .add_states(&[State::New, State::Running])
        .initial_state(State::New)
        .transition(State::New, Event::Start, State::Running)
        .cas_executor(executor)
        .build()
        .expect("valid machine");
    let state = AtomicRef::from_value(State::New);
    let mut observed = Vec::new();
    machine
        .trigger_with(&state, Event::Start, |old, new| observed.push((old, new)))
        .expect("start succeeds");
    assert_eq!(observed, vec![(State::New, State::Running)]);
    assert!(matches!(
        machine.trigger_with(&state, Event::Start, |_, _| panic!("must not run")),
        Err(StateMachineError::UnknownTransition { .. })
    ));
}

#[test]
fn test_concurrent_self_transitions_only_invoke_callbacks_for_commits() {
    let machine = StateMachine::builder()
        .add_state(State::New)
        .initial_state(State::New)
        .transition(State::New, Event::Start, State::New)
        .cas_executor(CasExecutor::builder().max_attempts(1).build().expect("single attempt"))
        .build()
        .expect("self transition");
    let state = AtomicRef::from_value(State::New);
    let callbacks = AtomicUsize::new(0);
    let gate = Barrier::new(4);
    let successes = thread::scope(|scope| {
        let mut threads = Vec::new();
        for _ in 0..4 {
            let machine = &machine;
            let state = &state;
            let callbacks = &callbacks;
            let gate = &gate;
            threads.push(scope.spawn(move || {
                gate.wait();
                let mut successes = 0;
                for _ in 0..100 {
                    if machine
                        .trigger_with(state, Event::Start, |old, next| {
                            assert_eq!(old, State::New);
                            assert_eq!(next, State::New);
                            callbacks.fetch_add(1, Ordering::SeqCst);
                        })
                        .is_ok()
                    {
                        successes += 1;
                    }
                }
                successes
            }));
        }
        threads
            .into_iter()
            .map(|thread| thread.join().expect("writer"))
            .sum::<usize>()
    });
    assert!(successes > 0);
    assert_eq!(callbacks.load(Ordering::SeqCst), successes);
    assert_eq!(*state.load(), State::New);
}
