// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]

//! Fuzzes bounded generic builder definitions and validates every constructed
//! transition target.

use libfuzzer_sys::fuzz_target;
use qubit_state_machine::StateMachine;

/// Limits builder work to at most 170 transition definitions per input.
const MAX_INPUT_BYTES: usize = 512;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_BYTES {
        return;
    }

    let state_count = data[0] % 16 + 1;
    let event_count = data[1] % 8 + 1;
    let states: Vec<u8> = (0..state_count).collect();
    let mut builder = StateMachine::builder().add_states(&states);

    for definition in data[2..].chunks_exact(3) {
        let source = definition[0] % (state_count + 2);
        let event = definition[1] % (event_count + 2);
        let target = definition[2] % (state_count + 2);
        builder = builder.transition(source, event, target);
    }

    let Ok(machine) = builder.build() else {
        return;
    };
    for source in 0..state_count {
        for event in 0..event_count {
            if let Some(target) = machine.transition_target(source, event) {
                assert!(target < state_count);
            }
        }
    }
});
