// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
#![cfg(feature = "fast")]
use qubit_fast_cas::FastCasState;
use qubit_state_machine::DenseCode;
use qubit_state_machine::TypedFastState;
use qubit_state_machine::TypedFastStateMachine;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Pending,
    Running,
    Done,
}
impl DenseCode for State {
    const VALUES: &'static [Self] = &[Self::Pending, Self::Running, Self::Done];
    fn code(self) -> u64 {
        self as u64
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Start,
    Finish,
}
impl DenseCode for Event {
    const VALUES: &'static [Self] = &[Self::Start, Self::Finish];
    fn code(self) -> u64 {
        self as u64
    }
}

#[test]
fn test_typed_machine_commits_enum_values() {
    let machine = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Pending)
        .terminal_state(State::Done)
        .transition(State::Pending, Event::Start, State::Running)
        .transition(State::Running, Event::Finish, State::Done)
        .build()
        .expect("typed lifecycle is valid");
    assert_eq!(machine.state_count(), 3);
    assert_eq!(machine.event_count(), 2);
    assert_eq!(machine.transition_count(), 2);
    let state = machine.create_state();
    assert_eq!(state.load(), State::Pending);
    assert_eq!(
        machine.trigger(&state, Event::Start).expect("typed lifecycle is valid"),
        State::Running
    );
    let mut seen = None;
    assert_eq!(
        machine
            .trigger_with(&state, Event::Finish, |old, new| seen = Some((old, new)))
            .expect("typed lifecycle is valid"),
        State::Done
    );
    assert_eq!(seen, Some((State::Running, State::Done)));
    assert!(machine.is_terminal_state(state.load()));
    assert!(!machine.try_trigger(&state, Event::Start));
}

/// Builds a lifecycle fixture with two distinct events.
fn create_machine() -> TypedFastStateMachine<State, Event> {
    TypedFastStateMachine::builder()
        .initial_state(State::Pending)
        .terminal_state(State::Done)
        .transition(State::Pending, Event::Start, State::Running)
        .transition(State::Running, Event::Finish, State::Done)
        .build()
        .expect("typed lifecycle is valid")
}

#[test]
fn test_typed_queries_policy_and_duplicate_edges() {
    use qubit_fast_cas::FastCasPolicy;
    use qubit_state_machine::Transition;
    use qubit_state_machine::TypedFastStateMachineBuilder;
    let machine = TypedFastStateMachineBuilder::<State, Event>::default()
        .initial_state(State::Running)
        .initial_state(State::Pending)
        .terminal_states(&[State::Done])
        .cas_policy(FastCasPolicy::spin(8))
        .transition_value(Transition::new(State::Pending, Event::Start, State::Running))
        .transition(State::Pending, Event::Start, State::Running)
        .build()
        .expect("duplicate edges are merged");
    assert_eq!(machine.cas_policy(), FastCasPolicy::spin(8));
    assert_eq!(machine.initial_state(), State::Pending);
    assert!(machine.is_initial_state(State::Pending));
    assert!(!machine.is_initial_state(State::Running));
    assert!(machine.contains_state(State::Done));
    assert_eq!(machine.terminal_states().collect::<Vec<_>>(), vec![State::Done]);
    assert_eq!(
        machine.transitions().collect::<Vec<_>>(),
        vec![Transition::new(State::Pending, Event::Start, State::Running)]
    );
    assert_eq!(
        machine.transition_target(State::Pending, Event::Start),
        Some(State::Running)
    );
    assert_eq!(machine.transition_target(State::Done, Event::Start), None);
    assert_eq!(machine.transition_count(), 1);
    let clone = machine.clone();
    assert_eq!(clone.initial_state(), machine.initial_state());
}

#[test]
fn test_typed_state_independence_and_same_state_type_other_machine() {
    let machine = create_machine();
    let first = machine.create_state();
    let second = machine.create_state();
    assert!(machine.try_trigger(&first, Event::Start));
    assert_eq!(first.load(), State::Running);
    assert_eq!(second.load(), State::Pending);
    let other = TypedFastStateMachine::<State, State>::builder()
        .initial_state(State::Pending)
        .transition(State::Running, State::Done, State::Done)
        .build()
        .expect("second rule set is valid");
    assert_eq!(
        other.trigger(&first, State::Done).expect("same state type is accepted"),
        State::Done
    );
    assert_eq!(std::mem::size_of_val(&first), std::mem::size_of::<FastCasState>());
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TypedFastState<State>>();
    assert_send_sync::<TypedFastStateMachine<State, Event>>();
}

#[test]
fn test_typed_callback_reentry_panic_and_failure() {
    use std::panic::AssertUnwindSafe;
    use std::panic::catch_unwind;
    let machine = create_machine();
    let state = machine.create_state();
    assert_eq!(
        machine
            .trigger_with(&state, Event::Start, |old, new| {
                assert_eq!((old, new), (State::Pending, State::Running));
                assert!(machine.try_trigger(&state, Event::Finish));
            })
            .expect("outer transition commits"),
        State::Running
    );
    assert_eq!(state.load(), State::Done);
    assert!(!machine.try_trigger_with(&state, Event::Start, |_, _| panic!("must not call")));
    let state = machine.create_state();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _result = machine.try_trigger_with(&state, Event::Start, |_, _| panic!("callback failure"));
        }))
        .is_err()
    );
    assert_eq!(state.load(), State::Running);
}

#[test]
fn test_typed_self_edge_commits_callback_once() {
    let machine = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Pending)
        .transition(State::Pending, Event::Start, State::Pending)
        .build()
        .expect("self edge is valid");
    let state = machine.create_state();
    let mut calls = 0;
    assert!(machine.try_trigger_with(&state, Event::Start, |old, new| {
        assert_eq!(old, new);
        calls += 1;
    }));
    assert_eq!(calls, 1);
    assert_eq!(state.load(), State::Pending);
}

#[test]
fn test_typed_builder_preserves_raw_errors() {
    use qubit_state_machine::FastStateMachineBuildError;
    use qubit_state_machine::TypedFastStateMachineBuildError;
    assert_eq!(
        TypedFastStateMachine::<State, Event>::builder()
            .build()
            .expect_err("missing initial"),
        TypedFastStateMachineBuildError::Raw(FastStateMachineBuildError::InitialStateNotConfigured)
    );
    for target in [State::Pending, State::Running] {
        assert!(matches!(
            TypedFastStateMachine::<State, Event>::builder()
                .initial_state(State::Pending)
                .terminal_state(State::Pending)
                .transition(State::Pending, Event::Start, target)
                .build(),
            Err(TypedFastStateMachineBuildError::Raw(
                FastStateMachineBuildError::TerminalStateHasOutgoingTransition { .. }
            ))
        ));
    }
    assert!(matches!(
        TypedFastStateMachine::<State, Event>::builder()
            .initial_state(State::Pending)
            .transition(State::Pending, Event::Start, State::Running)
            .transition(State::Pending, Event::Start, State::Done)
            .build(),
        Err(TypedFastStateMachineBuildError::Raw(
            FastStateMachineBuildError::DuplicateTransition { .. }
        ))
    ));
    let machine = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Done)
        .terminal_state(State::Done)
        .build()
        .expect("initial may be terminal");
    assert!(!machine.try_trigger(&machine.create_state(), Event::Finish));
}

#[test]
fn test_typed_builder_enforces_table_cell_limit() {
    use qubit_state_machine::FastStateMachineBuildError;
    use qubit_state_machine::TypedFastStateMachineBuildError;

    let rejected = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Pending)
        .max_table_cells(5)
        .build();
    assert_eq!(
        rejected.expect_err("three states and two events need six cells"),
        TypedFastStateMachineBuildError::Raw(FastStateMachineBuildError::TransitionTableLimitExceeded {
            state_count: 3,
            event_count: 2,
            cells: 6,
            limit: 5,
        })
    );

    let accepted = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Pending)
        .max_table_cells(6)
        .build()
        .expect("the inclusive limit permits six cells");
    assert_eq!(accepted.state_count(), 3);
    assert_eq!(accepted.event_count(), 2);
}

#[test]
fn test_typed_graph_diagnostics_reports_unreachable_and_nonterminal_states() {
    let machine = TypedFastStateMachine::<State, Event>::builder()
        .initial_state(State::Pending)
        .terminal_state(State::Done)
        .transition(State::Pending, Event::Finish, State::Done)
        .transition(State::Running, Event::Start, State::Running)
        .build()
        .expect("graph with an unreachable loop is valid");

    let report = machine.diagnose_graph();
    assert_eq!(report.unreachable_states(), &[State::Running]);
    assert_eq!(report.states_without_terminal_path(), &[State::Running]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Empty;
impl DenseCode for Empty {
    const VALUES: &'static [Self] = &[];
    fn code(self) -> u64 {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Broken<const CASE: u8>(u64);
impl<const CASE: u8> DenseCode for Broken<CASE> {
    const VALUES: &'static [Self] = match CASE {
        0 => &[Self(1), Self(0)],
        1 => &[Self(0), Self(0)],
        2 => &[Self(u64::MAX)],
        _ => &[Self(0), Self(1)],
    };
    fn code(self) -> u64 {
        let Self(value) = self;
        if CASE == 3 { 0 } else { value }
    }
}

#[test]
fn test_typed_invalid_codebooks_fail_before_rule_validation() {
    use qubit_state_machine::TypedFastStateMachineBuildError as Error;
    assert_eq!(
        TypedFastStateMachine::<Empty, Event>::builder()
            .build()
            .expect_err("empty states"),
        Error::EmptyCodebook { domain: "state" }
    );
    assert_eq!(
        TypedFastStateMachine::<State, Empty>::builder()
            .build()
            .expect_err("empty events"),
        Error::EmptyCodebook { domain: "event" }
    );
    assert!(matches!(
        TypedFastStateMachine::<Broken<0>, Event>::builder().build(),
        Err(Error::InvalidCodebook {
            domain: "state",
            index: 0,
            code: 1
        })
    ));
    assert!(matches!(
        TypedFastStateMachine::<State, Broken<1>>::builder().build(),
        Err(Error::InvalidCodebook {
            domain: "event",
            index: 1,
            code: 0
        })
    ));
    assert!(matches!(
        TypedFastStateMachine::<Broken<2>, Event>::builder().build(),
        Err(Error::InvalidCodebook { code: u64::MAX, .. })
    ));
    assert!(matches!(
        TypedFastStateMachine::<Broken<3>, Event>::builder().build(),
        Err(Error::InvalidCodebook { index: 1, code: 0, .. })
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Partial {
    Good,
    Missing,
    OutOfRange,
}
impl DenseCode for Partial {
    const VALUES: &'static [Self] = &[Self::Good];
    fn code(self) -> u64 {
        if self == Self::OutOfRange { u64::MAX } else { 0 }
    }
}

#[test]
fn test_typed_omitted_values_cannot_impersonate_registered_values() {
    use qubit_state_machine::TypedFastStateMachineBuildError;
    use qubit_state_machine::TypedFastStateMachineError;
    for value in [Partial::Missing, Partial::OutOfRange] {
        let expected = TypedFastStateMachineBuildError::ValueNotInCodebook {
            domain: "state",
            code: value.code(),
        };
        assert_eq!(
            TypedFastStateMachine::<Partial, Event>::builder()
                .initial_state(value)
                .build()
                .expect_err("omitted initial"),
            expected
        );
        assert_eq!(
            TypedFastStateMachine::<Partial, Event>::builder()
                .initial_state(Partial::Good)
                .terminal_state(value)
                .build()
                .expect_err("omitted terminal"),
            expected
        );
        assert_eq!(
            TypedFastStateMachine::<Partial, Event>::builder()
                .initial_state(Partial::Good)
                .transition(value, Event::Start, Partial::Good)
                .build()
                .expect_err("omitted source"),
            expected
        );
        assert_eq!(
            TypedFastStateMachine::<Partial, Event>::builder()
                .initial_state(Partial::Good)
                .transition(Partial::Good, Event::Start, value)
                .build()
                .expect_err("omitted target"),
            expected
        );
        let machine = TypedFastStateMachine::<Partial, Partial>::builder()
            .initial_state(Partial::Good)
            .transition(Partial::Good, Partial::Good, Partial::Good)
            .build()
            .expect("valid subset");
        assert!(!machine.contains_state(value));
        assert!(!machine.is_initial_state(value));
        assert!(!machine.is_terminal_state(value));
        assert_eq!(machine.transition_target(value, Partial::Good), None);
        assert_eq!(machine.transition_target(Partial::Good, value), None);
        let state = machine.create_state();
        let expected = TypedFastStateMachineError::InvalidEventCode {
            event: value,
            code: value.code(),
        };
        assert_eq!(machine.trigger(&state, value), Err(expected));
        assert_eq!(
            machine.trigger_with(&state, value, |_, _| panic!("invalid event callback")),
            Err(expected)
        );
        assert_eq!(state.load(), Partial::Good);
        assert!(matches!(
            TypedFastStateMachine::<State, Partial>::builder()
                .initial_state(State::Pending)
                .transition(State::Pending, value, State::Done)
                .build(),
            Err(TypedFastStateMachineBuildError::ValueNotInCodebook { domain: "event", .. })
        ));
    }
}

#[test]
fn test_typed_error_sources_and_diagnostics() {
    use std::error::Error;

    use qubit_state_machine::FastStateMachineBuildError;
    use qubit_state_machine::TypedFastStateMachineBuildError;
    use qubit_state_machine::TypedFastStateMachineError;
    let typed = TypedFastStateMachineError::UnknownTransition {
        source_state: State::Pending,
        event: Event::Finish,
    };
    assert_eq!(typed.to_string(), "unknown transition: Pending --Finish--> ?");
    assert!(typed.source().is_none());
    let raw = FastStateMachineBuildError::InitialStateNotConfigured;
    assert_eq!(TypedFastStateMachineBuildError::from(raw).to_string(), raw.to_string());
    assert_eq!(
        TypedFastStateMachineError::<State, Event>::InvalidEventCode {
            event: Event::Finish,
            code: 3
        }
        .to_string(),
        "event Finish with code 3 is not in its codebook"
    );
    assert_eq!(
        TypedFastStateMachineBuildError::EmptyCodebook { domain: "state" }.to_string(),
        "state codebook is empty"
    );
    assert_eq!(
        TypedFastStateMachineBuildError::InvalidCodebook {
            domain: "event",
            index: 1,
            code: 9
        }
        .to_string(),
        "event codebook index 1 has code 9"
    );
    assert_eq!(
        TypedFastStateMachineBuildError::ValueNotInCodebook {
            domain: "event",
            code: 9
        }
        .to_string(),
        "event value with code 9 is not in its codebook"
    );
}

#[test]
fn test_typed_error_classification() {
    use qubit_state_machine::TypedFastStateMachineError as Error;

    for (error, rejected, conflicted) in [
        (
            Error::InvalidEventCode {
                event: Event::Finish,
                code: 99,
            },
            false,
            false,
        ),
        (Error::InvalidStateCode { code: 99 }, false, false),
        (
            Error::UnknownTransition {
                source_state: State::Running,
                event: Event::Finish,
            },
            true,
            false,
        ),
        (Error::CasConflict { attempts: 7 }, false, true),
    ] {
        assert_eq!(error.is_unknown_transition(), rejected);
        assert_eq!(error.is_cas_conflict(), conflicted);
    }
}

#[test]
fn test_typed_callbacks_can_observe_later_state_and_finish_out_of_order() {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::mpsc;
    use std::time::Duration;
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
            .trigger_with(&thread_state, Event::Start, |old, new| {
                entered_tx.send(()).expect("receiver remains alive");
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("later callback releases earlier callback");
                assert_eq!((old, new), (State::Pending, State::Running));
                let state = &thread_state;
                assert_eq!(state.load(), State::Done);
                thread_order.lock().expect("order lock is healthy").push(0);
            })
            .expect("first transition commits")
    });
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("first callback entered");
    machine
        .trigger_with(&state, Event::Finish, |old, new| {
            assert_eq!((old, new), (State::Running, State::Done));
            order.lock().expect("order lock is healthy").push(1);
        })
        .expect("second transition commits");
    release_tx.send(()).expect("earlier callback is waiting");
    assert_eq!(worker.join().expect("worker finishes"), State::Running);
    assert_eq!(*order.lock().expect("order lock is healthy"), vec![1, 0]);
}
