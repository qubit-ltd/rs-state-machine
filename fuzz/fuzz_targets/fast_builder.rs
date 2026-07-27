// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]

//! Fuzzes bounded Fast builder definitions and validates every constructed
//! transition target.

use libfuzzer_sys::fuzz_target;
use qubit_state_machine::FastStateMachine;

/// Limits builder work to at most 170 transition definitions per input.
const MAX_INPUT_BYTES: usize = 512;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_BYTES {
        return;
    }

    let state_count = u64::from(data[0] % 16 + 1);
    let event_count = u64::from(data[1] % 8 + 1);
    let mut builder = FastStateMachine::builder()
        .state_count(state_count)
        .event_count(event_count);

    for definition in data[2..].chunks_exact(3) {
        let source = u64::from(definition[0]) % (state_count + 2);
        let event = u64::from(definition[1]) % (event_count + 2);
        let target = u64::from(definition[2]) % (state_count + 2);
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
