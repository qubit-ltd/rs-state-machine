// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]

//! Fuzzes arbitrary `u64` runtime state and event codes against a fixed Fast
//! state machine, including values that exceed 32-bit `usize`.

use libfuzzer_sys::fuzz_target;
use qubit_fast_cas::FastCasState;
use qubit_state_machine::FastStateMachine;
use qubit_state_machine::FastStateMachineError;

/// A runtime input contains exactly one state code and one event code.
const INPUT_BYTES: usize = 16;

fuzz_target!(|data: &[u8]| {
    if data.len() != INPUT_BYTES {
        return;
    }

    let mut state_bytes = [0_u8; 8];
    state_bytes.copy_from_slice(&data[..8]);
    let current = u64::from_le_bytes(state_bytes);
    let mut event_bytes = [0_u8; 8];
    event_bytes.copy_from_slice(&data[8..]);
    let event = u64::from_le_bytes(event_bytes);

    let machine = FastStateMachine::builder()
        .state_count(2)
        .event_count(1)
        .transition(0, 0, 1)
        .transition(1, 0, 0)
        .build()
        .expect("fixed Fast fuzzing fixture should build");
    let state = FastCasState::new(current);
    let result = machine.trigger(&state, event);

    if current >= 2 {
        assert_eq!(result, Err(FastStateMachineError::UnknownState { state: current }));
        assert_eq!(state.load(), current);
    } else if event != 0 {
        assert_eq!(
            result,
            Err(FastStateMachineError::UnknownTransition {
                source_state: current,
                event,
            })
        );
        assert_eq!(state.load(), current);
    } else {
        let expected = 1 - current;
        assert_eq!(result, Ok(expected));
        assert_eq!(state.load(), expected);
    }
});
