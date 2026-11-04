// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic retry behavior under a competing state transition.

use std::cell::RefCell;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::mpsc::channel;
use std::thread::spawn;
use std::time::Duration;

use qubit_atomic::AtomicRef;
use qubit_cas::CasErrorKind;
use qubit_cas::CasExecutor;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineError;

const CHANNEL_TIMEOUT: Duration = Duration::from_secs(5);
type TriggerResult = Result<State, StateMachineError<State, Event>>;
type InterleavingResult = (TriggerResult, Vec<(State, State)>, usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum State {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Work,
    Advance,
}

/// Pauses the worker at its first decision lookup until the competing commit
/// finishes.
struct HashGate {
    work_hashes: usize,
    entered: Sender<()>,
    release: Receiver<()>,
}

thread_local! {
    static HASH_GATE: RefCell<Option<HashGate>> = const { RefCell::new(None) };
}

impl Hash for Event {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // The synchronization is deliberately attached to the second Work
        // hash: one hash is the preflight lookup, then the executor has loaded
        // its A snapshot and is consulting the transition map for its decision.
        HASH_GATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(gate) = slot.as_mut()
                && matches!(self, Self::Work)
            {
                gate.work_hashes += 1;
                if gate.work_hashes == 2 {
                    gate.entered.send(()).expect("main thread awaits hash gate");
                    gate.release
                        .recv_timeout(CHANNEL_TIMEOUT)
                        .expect("main thread releases hash gate before timeout");
                }
            }
        });
        state.write_u8(match self {
            Self::Work => 0,
            Self::Advance => 1,
        });
    }
}

/// Builds a bounded retry scenario, optionally allowing Work after the
/// competing commit.
fn machine(include_retried_work: bool, max_attempts: u32) -> StateMachine<State, Event> {
    let executor = CasExecutor::<State, StateMachineError<State, Event>>::builder()
        .max_attempts(max_attempts)
        .max_operation_elapsed(None)
        .max_total_elapsed(None)
        .flow_timeout(None)
        .attempt_timeout(None)
        .no_delay()
        .build()
        .expect("positive immediate attempt budget is valid");
    let builder = StateMachine::builder()
        .add_states(&[State::A, State::B, State::C, State::D])
        .initial_state(State::A)
        .transition(State::A, Event::Work, State::C)
        .transition(State::A, Event::Advance, State::B)
        .cas_executor(executor);
    let builder = if include_retried_work {
        builder.transition(State::B, Event::Work, State::D)
    } else {
        builder
    };
    builder.build().expect("test transition rules are valid")
}

/// Commits Advance while Work is paused, then collects its result and callback
/// history.
fn run_interleaving(machine: Arc<StateMachine<State, Event>>, state: Arc<AtomicRef<State>>) -> InterleavingResult {
    let (entered_tx, entered_rx) = channel();
    let (release_tx, release_rx) = channel();
    let (result_tx, result_rx) = channel();
    let worker_machine = Arc::clone(&machine);
    let worker_state = Arc::clone(&state);
    let worker = spawn(move || {
        HASH_GATE.with(|slot| {
            *slot.borrow_mut() = Some(HashGate {
                work_hashes: 0,
                entered: entered_tx,
                release: release_rx,
            });
        });
        let mut callback_args = Vec::new();
        let result = worker_machine.trigger_with(&worker_state, Event::Work, |from, to| {
            callback_args.push((from, to));
        });
        let work_hashes = HASH_GATE.with(|slot| {
            slot.borrow_mut()
                .take()
                .expect("worker installed hash gate")
                .work_hashes
        });
        result_tx
            .send((result, callback_args, work_hashes))
            .expect("main thread receives worker result");
    });

    entered_rx
        .recv_timeout(CHANNEL_TIMEOUT)
        .expect("worker reaches controlled decision point");
    assert_eq!(
        machine.trigger(&state, Event::Advance),
        Ok(State::B),
        "competing transition advances A to B"
    );
    release_tx.send(()).expect("worker waits at hash gate");
    let result = result_rx
        .recv_timeout(CHANNEL_TIMEOUT)
        .expect("worker reports result after release");
    worker.join().expect("worker thread completes");
    result
}

#[test]
fn test_retry_rejects_event_invalidated_by_competing_commit() {
    let machine = Arc::new(machine(false, 2));
    let state = Arc::new(AtomicRef::from_value(State::A));

    let (result, callback_args, work_hashes) = run_interleaving(machine, Arc::clone(&state));

    assert_eq!(
        result,
        Err(StateMachineError::UnknownTransition {
            source_state: State::B,
            event: Event::Work,
        }),
        "retry must validate Work against the competing commit"
    );
    assert_eq!(
        *state.load(),
        State::B,
        "the rejected retry preserves the competing commit"
    );
    assert!(
        callback_args.is_empty(),
        "a rejected retry must not invoke the callback"
    );
    assert!(work_hashes >= 3, "expected preflight, decision, and retry queries");
}

#[test]
fn test_retry_recomputes_target_after_competing_commit() {
    let machine = Arc::new(machine(true, 2));
    let state = Arc::new(AtomicRef::from_value(State::A));

    let (result, callback_args, work_hashes) = run_interleaving(machine, Arc::clone(&state));

    assert_eq!(result, Ok(State::D), "retry must recompute the target from B");
    assert_eq!(*state.load(), State::D, "the recomputed target must be committed");
    assert_eq!(
        callback_args,
        [(State::B, State::D)],
        "only the successful retry invokes the callback"
    );
    assert!(work_hashes >= 3, "expected preflight, decision, and retry queries");
}

#[test]
fn test_single_attempt_conflict_preserves_error_and_skips_callback() {
    let machine = Arc::new(machine(true, 1));
    let state = Arc::new(AtomicRef::from_value(State::A));
    let (result, callback_args, work_hashes) = run_interleaving(machine, Arc::clone(&state));

    assert_eq!(
        result,
        Err(StateMachineError::CasFailure {
            kind: CasErrorKind::ConflictExhausted,
            attempts: 1,
        }),
        "the public trigger preserves the terminal CAS failure"
    );
    assert_eq!(*state.load(), State::B, "the competing commit remains installed");
    assert!(callback_args.is_empty(), "a failed commit must not call its callback");
    assert_eq!(work_hashes, 2, "one attempt must not enter a retry decision");
}
