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
    transitions: Vec<(u64, u64, u64)>,
}

/// Decodes 2..=512 bytes, retaining out-of-range codes for error checks.
fn definition(data: &[u8]) -> Option<Definition> {
    if !(2..=512).contains(&data.len()) {
        return None;
    }
    let state_count = u64::from(data[0] % 16 + 1);
    let event_count = u64::from(data[1] % 8 + 1);
    let transitions = data[2..]
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
        transitions,
    }) = definition(data)
    else {
        return BuilderOutcome::Skipped;
    };
    let mut builder = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count)
        .initial_state(0);
    for &(source, event, target) in &transitions {
        builder = builder.transition(source, event, target);
    }
    let mut model = BTreeMap::new();
    let mut expected_error = None;
    for &(source, event, target) in &transitions {
        let error = if source >= state_count {
            Some(Error::TransitionSourceOutOfRange {
                source_state: source,
                state_count,
            })
        } else if event >= event_count {
            Some(Error::TransitionEventOutOfRange { event, event_count })
        } else if target >= state_count {
            Some(Error::TransitionTargetOutOfRange { target, state_count })
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
    let state = machine.create_state();
    for event in 0..event_count + 2 {
        // Each probe starts at the real initial state instead of bypassing the machine.
        let state = machine.create_state();
        let expected = model.get(&(0, event)).copied();
        assert_eq!(machine.trigger(&state, event).ok(), expected);
    }
    assert_eq!(state.load(), 0);
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
        transitions,
    }) = definition(data)
    else {
        return BuilderOutcome::Skipped;
    };
    let states: Vec<u64> = (0..state_count).collect();
    let mut builder = StateMachine::builder().add_states(&states).initial_state(0);
    for &(source, event, target) in &transitions {
        builder = builder.transition(source, event, target);
    }
    let mut model = BTreeMap::new();
    let mut expected_error = None;
    for &(source, event, target) in &transitions {
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
    let state = machine.create_state();
    for event in 0..event_count + 2 {
        // Each probe starts at the real initial state instead of bypassing the machine.
        let state = machine.create_state();
        let expected = model.get(&(0, event)).copied();
        assert_eq!(machine.trigger(&state, event).ok(), expected);
    }
    assert_eq!(*state.load(), 0);
    BuilderOutcome::Built {
        checked_pairs: (state_count * event_count) as usize,
    }
}
