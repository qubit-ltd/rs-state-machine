// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Compares runtime traces against a dense reference model, independently of
//! CAS.
use qubit_cas::CasExecutor;
use qubit_state_machine::DenseCode;
use qubit_state_machine::FastStateMachine;
use qubit_state_machine::FastStateMachineBuildError;
use qubit_state_machine::FastStateMachineError;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineBuildError;
use qubit_state_machine::StateMachineError;
use qubit_state_machine::TypedFastState;
use qubit_state_machine::TypedFastStateMachine;
use qubit_state_machine::TypedFastStateMachineBuildError;
use qubit_state_machine::TypedFastStateMachineError;

use super::builder_cases::BuilderOutcome;

/// Fixed typed state domain; generated traces only visit their declared subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffState {
    S0,
    S1,
    S2,
    S3,
}
impl DenseCode for DiffState {
    const VALUES: &'static [Self] = &[Self::S0, Self::S1, Self::S2, Self::S3];
    #[inline]
    fn code(self) -> u64 {
        self as u64
    }
}

/// Fixed typed event domain, including an always-undefined sentinel event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffEvent {
    E0,
    E1,
    E2,
    Invalid,
    OmittedAlias,
    OmittedOutOfRange,
}
impl DenseCode for DiffEvent {
    const VALUES: &'static [Self] = &[Self::E0, Self::E1, Self::E2, Self::Invalid];
    #[inline]
    fn code(self) -> u64 {
        match self {
            Self::E0 | Self::OmittedAlias => 0,
            Self::E1 => 1,
            Self::E2 => 2,
            Self::Invalid => 3,
            Self::OmittedOutOfRange => u64::MAX,
        }
    }
}

/// Checks that an event outside the declared codebook is rejected without
/// invoking callbacks or changing the state through any typed trigger API.
fn check_invalid_event(
    machine: &TypedFastStateMachine<DiffState, DiffEvent>,
    state: &TypedFastState<DiffState>,
    event: DiffEvent,
) {
    let before = state.load();
    let expected = TypedFastStateMachineError::InvalidEventCode {
        event,
        code: event.code(),
    };
    assert_eq!(machine.trigger(state, event), Err(expected));
    let mut callbacks = 0;
    assert_eq!(machine.trigger_with(state, event, |_, _| callbacks += 1), Err(expected));
    assert!(!machine.try_trigger(state, event));
    assert!(!machine.try_trigger_with(state, event, |_, _| callbacks += 1));
    assert_eq!(callbacks, 0);
    assert_eq!(state.load(), before);
}

/// Checks a bounded table and every event in its trailing trace.
pub fn run_differential(data: &[u8]) -> BuilderOutcome {
    if !(2..=512).contains(&data.len()) {
        return BuilderOutcome::Skipped;
    }
    let extended = data[0] == u8::MAX && data.len() >= 7;
    if data[0] == u8::MAX && data.len() < 7 {
        return BuilderOutcome::Skipped;
    }
    let (state_count, event_count, initial_state, terminal_states, terminals_first, offset) = if extended {
        let state_count = u64::from(data[1] % 4 + 1);
        let event_count = u64::from(data[2] % 3 + 1);
        let mask = u16::from_le_bytes([data[4], data[5]]);
        (
            state_count,
            event_count,
            u64::from(data[3]) % state_count,
            (0..state_count)
                .filter(|&state| mask & (1_u16 << state) != 0)
                .collect::<Vec<_>>(),
            data[6] & 1 == 0,
            7,
        )
    } else {
        (
            u64::from(data[0] % 4 + 1),
            u64::from(data[1] % 3 + 1),
            0,
            Vec::new(),
            true,
            2,
        )
    };
    let cells = (state_count * event_count) as usize;
    if data.len() < offset + cells {
        return BuilderOutcome::Skipped;
    }
    let table: Vec<Option<u64>> = data[offset..offset + cells]
        .iter()
        .map(|&code| (code != u8::MAX).then_some(u64::from(code) % state_count))
        .collect();
    let states: Vec<u64> = (0..state_count).collect();
    let mut fast = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count)
        .initial_state(initial_state);
    let executor = CasExecutor::<u64, StateMachineError<u64, u64>>::builder()
        .max_attempts(1)
        .max_operation_elapsed(None)
        .max_total_elapsed(None)
        .attempt_timeout(None)
        .flow_timeout(None)
        .no_delay()
        .build()
        .expect("deterministic differential executor configuration");
    let mut standard = StateMachine::builder()
        .add_states(&states)
        .initial_state(initial_state)
        .cas_executor(executor);
    let typed_terminals: Vec<DiffState> = terminal_states
        .iter()
        .map(|&state| DiffState::VALUES[state as usize])
        .collect();
    let mut typed = TypedFastStateMachine::builder().initial_state(DiffState::VALUES[initial_state as usize]);
    if terminals_first {
        fast = fast.terminal_states(&terminal_states);
        standard = standard.terminal_states(&terminal_states);
        typed = typed.terminal_states(&typed_terminals);
    }
    for source in 0..state_count {
        for event in 0..event_count {
            if let Some(target) = table[(source * event_count + event) as usize] {
                fast = fast.transition(source, event, target);
                standard = standard.transition(source, event, target);
                typed = typed.transition(
                    DiffState::VALUES[source as usize],
                    DiffEvent::VALUES[event as usize],
                    DiffState::VALUES[target as usize],
                );
            }
        }
    }
    if !terminals_first {
        fast = fast.terminal_states(&terminal_states);
        standard = standard.terminal_states(&terminal_states);
        typed = typed.terminal_states(&typed_terminals);
    }
    let outgoing = (0..state_count).find_map(|source| {
        (0..event_count).find_map(|event| {
            if terminal_states.contains(&source) {
                table[(source * event_count + event) as usize].map(|target| (source, event, target))
            } else {
                None
            }
        })
    });
    if let Some((source, event, target)) = outgoing {
        assert_eq!(
            fast.build().unwrap_err(),
            FastStateMachineBuildError::TerminalStateHasOutgoingTransition {
                state: source,
                event,
                target
            }
        );
        assert_eq!(
            standard.build().unwrap_err(),
            StateMachineBuildError::TerminalStateHasOutgoingTransition {
                state: source,
                event,
                target
            }
        );
        assert_eq!(
            typed.build().unwrap_err(),
            TypedFastStateMachineBuildError::Raw(FastStateMachineBuildError::TerminalStateHasOutgoingTransition {
                state: source,
                event,
                target
            })
        );
        return BuilderOutcome::Rejected;
    }
    let fast = fast.build().expect("generated dense table is valid");
    let standard = standard.build().expect("generated generic table is valid");
    assert_eq!(fast.transition_count(), standard.transition_count());
    let typed = typed.build().expect("typed reference table is valid");
    assert_eq!(typed.transition_count(), fast.transition_count());
    assert_eq!(fast.initial_state(), initial_state);
    assert_eq!(standard.initial_state(), initial_state);
    assert_eq!(typed.initial_state(), DiffState::VALUES[initial_state as usize]);
    for state in 0..state_count {
        let terminal = terminal_states.contains(&state);
        assert_eq!(fast.is_terminal_state(state), terminal);
        assert_eq!(standard.is_terminal_state(state), terminal);
        assert_eq!(typed.is_terminal_state(DiffState::VALUES[state as usize]), terminal);
    }
    let mut checked_pairs = 0;
    for source in 0..state_count {
        for event in 0..event_count {
            let expected = table[(source * event_count + event) as usize];
            assert_eq!(
                standard.transition_target(source, event),
                expected,
                "Standard query: source={source}, event={event}"
            );
            assert_eq!(
                fast.transition_target(source, event),
                expected,
                "Raw Fast query: source={source}, event={event}"
            );
            assert_eq!(
                typed
                    .transition_target(DiffState::VALUES[source as usize], DiffEvent::VALUES[event as usize],)
                    .map(DenseCode::code),
                expected,
                "Typed Fast query: source={source}, event={event}"
            );
            checked_pairs += 1;
        }
    }
    let typed_state = typed.create_state();
    let fast_state = fast.create_state();
    let standard_state = standard.create_state();
    let mut current = initial_state;
    check_invalid_event(&typed, &typed_state, DiffEvent::OmittedAlias);
    check_invalid_event(&typed, &typed_state, DiffEvent::OmittedOutOfRange);
    for &byte in &data[offset + cells..] {
        if byte == 254 || byte == 255 {
            let invalid = if byte == 254 {
                DiffEvent::OmittedAlias
            } else {
                DiffEvent::OmittedOutOfRange
            };
            check_invalid_event(&typed, &typed_state, invalid);
            assert_eq!(typed_state.load().code(), current);
            assert_eq!(fast_state.load(), current);
            assert_eq!(*standard_state.load(), current);
            continue;
        }
        let event = u64::from(byte) % (event_count + 1);
        let expected = if event < event_count {
            table[(current * event_count + event) as usize]
        } else {
            None
        };
        let mut fast_callback = Vec::new();
        let mut standard_callback = Vec::new();
        let mut typed_callback = Vec::new();
        let actual_typed = typed
            .trigger_with(&typed_state, DiffEvent::VALUES[event as usize], |old, new| {
                typed_callback.push((old.code(), new.code()))
            })
            .map(DenseCode::code);
        let actual_fast = fast.trigger_with(&fast_state, event, |old, new| fast_callback.push((old, new)));
        let actual_standard =
            standard.trigger_with(&standard_state, event, |old, new| standard_callback.push((old, new)));
        if let Some(target) = expected {
            assert_eq!(actual_fast, Ok(target));
            assert_eq!(actual_typed, Ok(target));
            assert_eq!(actual_standard, Ok(target));
            assert_eq!(fast_callback, vec![(current, target)]);
            current = target;
        } else {
            assert_eq!(
                actual_fast,
                Err(FastStateMachineError::UnknownTransition {
                    source_state: current,
                    event
                })
            );
            assert_eq!(
                actual_standard,
                Err(StateMachineError::UnknownTransition {
                    source_state: current,
                    event
                })
            );
            assert_eq!(
                actual_typed,
                Err(TypedFastStateMachineError::UnknownTransition {
                    source_state: DiffState::VALUES[current as usize],
                    event: DiffEvent::VALUES[event as usize],
                })
            );
            assert!(fast_callback.is_empty());
            assert!(standard_callback.is_empty());
            assert!(typed_callback.is_empty());
        }
        assert_eq!(fast_callback, standard_callback);
        assert_eq!(typed_callback, standard_callback);
        assert_eq!(typed_state.load().code(), current);
        assert_eq!(fast_state.load(), current);
        assert_eq!(*standard_state.load(), current);
    }
    BuilderOutcome::Built { checked_pairs }
}
