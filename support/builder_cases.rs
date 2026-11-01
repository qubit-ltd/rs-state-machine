// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Independent ordered-map models for bounded builder definitions.
use std::collections::BTreeMap;

#[cfg(feature = "fast")]
use qubit_state_machine::FastStateMachineError;
#[cfg(feature = "standard")]
use qubit_state_machine::StateMachineError;

/// Records whether an input reached post-build assertions.
#[derive(Debug, PartialEq, Eq)]
pub enum BuilderOutcome {
    Skipped,
    Rejected,
    Built { checked_pairs: usize },
}

/// Bounded dimensions and definitions decoded independently of the builders.
struct Definition {
    state_count: u64,
    event_count: u64,
    initial_state: u64,
    terminal_states: Vec<u64>,
    terminals_first: bool,
    transitions: Vec<(u64, u64, u64)>,
}

/// Decodes 2..=512 bytes, retaining out-of-range codes for error checks.
fn definition(data: &[u8]) -> Option<Definition> {
    if !(2..=512).contains(&data.len()) {
        return None;
    }
    if data[0] == u8::MAX && data.len() < 7 {
        return None;
    }
    let extended = data[0] == u8::MAX && data.len() >= 7;
    let (state_count, event_count, initial_state, terminal_states, terminals_first, offset) = if extended {
        let state_count = u64::from(data[1] % 16 + 1);
        let event_count = u64::from(data[2] % 8 + 1);
        let mask = u16::from_le_bytes([data[4], data[5]]);
        (
            state_count,
            event_count,
            u64::from(data[3]) % (state_count + 2),
            (0_u64..16).filter(|&state| mask & (1_u16 << state) != 0).collect(),
            data[6] & 1 == 0,
            7,
        )
    } else {
        (
            u64::from(data[0] % 16 + 1),
            u64::from(data[1] % 8 + 1),
            0,
            Vec::new(),
            true,
            2,
        )
    };
    let transitions = data[offset..]
        .chunks_exact(3)
        .map(|triple| {
            (
                u64::from(triple[0]) % (state_count + 2),
                u64::from(triple[1]) % (event_count + 2),
                u64::from(triple[2]) % (state_count + 2),
            )
        })
        .collect();
    Some(Definition {
        state_count,
        event_count,
        initial_state,
        terminal_states,
        terminals_first,
        transitions,
    })
}

/// Checks every accepted edge or the first expected rejection for the fast
/// backend.
#[cfg(feature = "fast")]
pub fn run_fast_builder(data: &[u8]) -> BuilderOutcome {
    use qubit_state_machine::FastStateMachine;
    use qubit_state_machine::FastStateMachineBuildError as Error;
    let Some(Definition {
        state_count,
        event_count,
        initial_state,
        terminal_states,
        terminals_first,
        transitions,
    }) = definition(data)
    else {
        return BuilderOutcome::Skipped;
    };
    let mut builder = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count)
        .initial_state(initial_state);
    if terminals_first {
        builder = builder.terminal_states(&terminal_states);
    }
    for &(source, event, target) in &transitions {
        builder = builder.transition(source, event, target);
    }
    if !terminals_first {
        builder = builder.terminal_states(&terminal_states);
    }
    let terminals: std::collections::BTreeSet<u64> = terminal_states.iter().copied().collect();
    let mut model = BTreeMap::new();
    let mut expected_error = if initial_state >= state_count {
        Some(Error::InitialStateOutOfRange {
            state: initial_state,
            state_count,
        })
    } else {
        terminal_states
            .iter()
            .find(|&&state| state >= state_count)
            .map(|&state| Error::TerminalStateOutOfRange { state, state_count })
    };
    for &(source, event, target) in &transitions {
        if expected_error.is_some() {
            break;
        }
        let error = if source >= state_count {
            Some(Error::TransitionSourceOutOfRange {
                source_state: source,
                state_count,
            })
        } else if event >= event_count {
            Some(Error::TransitionEventOutOfRange { event, event_count })
        } else if target >= state_count {
            Some(Error::TransitionTargetOutOfRange { target, state_count })
        } else if terminals.contains(&source) {
            Some(Error::TerminalStateHasOutgoingTransition {
                state: source,
                event,
                target,
            })
        } else if let Some(&existing_target) = model.get(&(source, event)) {
            (existing_target != target).then_some(Error::DuplicateTransition {
                source_state: source,
                event,
                existing_target,
                new_target: target,
            })
        } else {
            None
        };
        if error.is_some() {
            expected_error = error;
            break;
        }
        model.insert((source, event), target);
    }
    let machine = match (expected_error, builder.build()) {
        (Some(expected), Err(actual)) => {
            assert_eq!(actual, expected);
            return BuilderOutcome::Rejected;
        }
        (None, Ok(machine)) => machine,
        (expected, actual) => panic!("builder disagrees with reference model: {expected:?}, {actual:?}"),
    };
    assert_eq!(machine.transition_count(), model.len());
    for source in 0..state_count {
        for event in 0..event_count + 2 {
            assert_eq!(
                machine.transition_target(source, event),
                model.get(&(source, event)).copied()
            );
        }
    }
    assert_eq!(machine.initial_state(), initial_state);
    for state in 0..state_count {
        assert_eq!(machine.is_terminal_state(state), terminals.contains(&state));
    }
    let unchanged_state = machine.create_state();
    for event in 0..event_count + 2 {
        // Each probe starts at the real initial state instead of bypassing the machine.
        let probe_state = machine.create_state();
        let expected = model.get(&(initial_state, event)).copied();
        let mut callbacks = Vec::new();
        assert_eq!(
            machine.trigger_with(&probe_state, event, |old, new| callbacks.push((old, new))),
            expected.ok_or(FastStateMachineError::UnknownTransition {
                source_state: initial_state,
                event,
            })
        );
        assert_eq!(
            callbacks,
            expected.map(|target| vec![(initial_state, target)]).unwrap_or_default()
        );
        assert_eq!(probe_state.load(), expected.unwrap_or(initial_state));
    }
    assert_eq!(unchanged_state.load(), initial_state);
    BuilderOutcome::Built {
        checked_pairs: (state_count * event_count) as usize,
    }
}

/// Checks every accepted edge or the first expected rejection for the standard
/// backend.
#[cfg(feature = "standard")]
pub fn run_standard_builder(data: &[u8]) -> BuilderOutcome {
    use qubit_state_machine::StateMachine;
    use qubit_state_machine::StateMachineBuildError as Error;
    let Some(Definition {
        state_count,
        event_count,
        initial_state,
        terminal_states,
        terminals_first,
        transitions,
    }) = definition(data)
    else {
        return BuilderOutcome::Skipped;
    };
    let states: Vec<u64> = (0..state_count).collect();
    let mut builder = StateMachine::builder().add_states(&states).initial_state(initial_state);
    if terminals_first {
        builder = builder.terminal_states(&terminal_states);
    }
    for &(source, event, target) in &transitions {
        builder = builder.transition(source, event, target);
    }
    if !terminals_first {
        builder = builder.terminal_states(&terminal_states);
    }
    let terminals: std::collections::BTreeSet<u64> = terminal_states.iter().copied().collect();
    let mut model = BTreeMap::new();
    let mut expected_error = if initial_state >= state_count {
        Some(Error::InitialStateNotRegistered { state: initial_state })
    } else {
        terminal_states
            .iter()
            .find(|&&state| state >= state_count)
            .map(|&state| Error::TerminalStateNotRegistered { state })
    };
    for &(source, event, target) in &transitions {
        if expected_error.is_some() {
            break;
        }
        let error = if source >= state_count {
            Some(Error::TransitionSourceNotRegistered {
                source_state: source,
                event,
                target,
            })
        } else if target >= state_count {
            Some(Error::TransitionTargetNotRegistered {
                source_state: source,
                event,
                target,
            })
        } else if terminals.contains(&source) {
            Some(Error::TerminalStateHasOutgoingTransition {
                state: source,
                event,
                target,
            })
        } else if let Some(&existing_target) = model.get(&(source, event)) {
            (existing_target != target).then_some(Error::DuplicateTransition {
                source_state: source,
                event,
                existing_target,
                new_target: target,
            })
        } else {
            None
        };
        if error.is_some() {
            expected_error = error;
            break;
        }
        model.insert((source, event), target);
    }
    let machine = match (expected_error, builder.build()) {
        (Some(expected), Err(actual)) => {
            assert_eq!(actual, expected);
            return BuilderOutcome::Rejected;
        }
        (None, Ok(machine)) => machine,
        (expected, actual) => panic!("builder disagrees with reference model: {expected:?}, {actual:?}"),
    };
    assert_eq!(machine.transition_count(), model.len());
    for source in 0..state_count {
        for event in 0..event_count + 2 {
            assert_eq!(
                machine.transition_target(source, event),
                model.get(&(source, event)).copied()
            );
        }
    }
    assert_eq!(machine.initial_state(), initial_state);
    for state in 0..state_count {
        assert_eq!(machine.is_terminal_state(state), terminals.contains(&state));
    }
    let unchanged_state = machine.create_state();
    for event in 0..event_count + 2 {
        // Each probe starts at the real initial state instead of bypassing the machine.
        let probe_state = machine.create_state();
        let expected = model.get(&(initial_state, event)).copied();
        let mut callbacks = Vec::new();
        assert_eq!(
            machine.trigger_with(&probe_state, event, |old, new| callbacks.push((old, new))),
            expected.ok_or(StateMachineError::UnknownTransition {
                source_state: initial_state,
                event,
            })
        );
        assert_eq!(
            callbacks,
            expected.map(|target| vec![(initial_state, target)]).unwrap_or_default()
        );
        assert_eq!(*probe_state.load(), expected.unwrap_or(initial_state));
    }
    assert_eq!(*unchanged_state.load(), initial_state);
    BuilderOutcome::Built {
        checked_pairs: (state_count * event_count) as usize,
    }
}
