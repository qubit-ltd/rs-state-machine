// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
#![no_main]
//! Exercises the shared fast builder model.
use libfuzzer_sys::fuzz_target;
#[allow(dead_code, unused_imports)]
#[path = "../support/mod.rs"]
mod support;
fuzz_target!(|data: &[u8]| {
    let _outcome = support::run_fast_builder(data);
});
