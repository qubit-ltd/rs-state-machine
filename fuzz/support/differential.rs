// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Compares runtime traces against a dense reference model, independently of
//! CAS.
use qubit_state_machine::DenseCode;
use qubit_state_machine::FastStateMachine;
use qubit_state_machine::FastStateMachineError;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineError;
use qubit_state_machine::TypedFastStateMachine;
use qubit_state_machine::TypedFastStateMachineError;

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
}
impl DenseCode for DiffEvent {
    const VALUES: &'static [Self] = &[Self::E0, Self::E1, Self::E2, Self::Invalid];
    fn code(self) -> u64 {
        self as u64
    }
}

/// Checks a bounded table and every event in its trailing trace.
pub fn run_differential(data: &[u8]) {
    if !(2..=512).contains(&data.len()) {
        return;
    }
    let state_count = u64::from(data[0] % 4 + 1);
    let event_count = u64::from(data[1] % 3 + 1);
    let cells = (state_count * event_count) as usize;
    if data.len() < 2 + cells {
        return;
    }
    let table: Vec<Option<u64>> = data[2..2 + cells]
        .iter()
        .map(|&code| (code != u8::MAX).then_some(u64::from(code) % state_count))
        .collect();
    let states: Vec<u64> = (0..state_count).collect();
    let mut fast = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count)
        .initial_state(0);
    let mut standard = StateMachine::builder().add_states(&states).initial_state(0);
    let mut typed = TypedFastStateMachine::builder().initial_state(DiffState::S0);
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
    let fast = fast.build().expect("generated dense table is valid");
    let standard = standard.build().expect("generated generic table is valid");
    assert_eq!(fast.transition_count(), standard.transition_count());
    let typed = typed.build().expect("typed reference table is valid");
    assert_eq!(typed.transition_count(), fast.transition_count());
    let typed_state = typed.create_state();
    let fast_state = fast.create_state();
    let standard_state = standard.create_state();
    let mut current = 0;
    for &byte in &data[2 + cells..] {
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
                Err(TypedFastStateMachineError::Raw(
                    FastStateMachineError::UnknownTransition {
                        source_state: current,
                        event
                    }
                ))
            );
            assert!(fast_callback.is_empty());
        }
        assert_eq!(fast_callback, standard_callback);
        assert_eq!(typed_callback, standard_callback);
        assert_eq!(typed_state.load().code(), current);
        assert_eq!(fast_state.load(), current);
        assert_eq!(*standard_state.load(), current);
    }
}
