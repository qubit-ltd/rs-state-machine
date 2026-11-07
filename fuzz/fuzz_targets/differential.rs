// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Fuzzes runtime equivalence with an independent reference state.

#![no_main]

use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../support/mod.rs"]
mod support;

fuzz_target!(|data: &[u8]| {
    support::differential::run_differential(data);
});
