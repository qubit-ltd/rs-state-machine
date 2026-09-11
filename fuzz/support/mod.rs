// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Shared bounded fuzz harnesses exercised by ordinary regression tests.
mod builder_cases;
pub type BuilderOutcome = builder_cases::BuilderOutcome;
#[cfg(feature = "fast")]
pub fn run_fast_builder(data: &[u8]) -> BuilderOutcome {
    builder_cases::run_fast_builder(data)
}
#[cfg(feature = "standard")]
pub fn run_standard_builder(data: &[u8]) -> BuilderOutcome {
    builder_cases::run_standard_builder(data)
}

#[cfg(all(feature = "fast", feature = "standard"))]
mod differential;
#[cfg(all(feature = "fast", feature = "standard"))]
pub fn run_differential(data: &[u8]) {
    differential::run_differential(data);
}
