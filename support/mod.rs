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
pub use builder_cases::BuilderOutcome;
#[cfg(feature = "fast")]
pub use builder_cases::run_fast_builder;
#[cfg(feature = "standard")]
pub use builder_cases::run_standard_builder;

#[cfg(all(feature = "fast", feature = "standard"))]
mod differential;
#[cfg(all(feature = "fast", feature = "standard"))]
pub use differential::run_differential;
