// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Shared bounded fuzz harnesses exercised by ordinary regression tests.
pub(crate) mod builder_cases;
#[cfg(test)]
pub use builder_cases::BuilderOutcome;
#[cfg(all(test, feature = "fast"))]
pub use builder_cases::run_fast_builder;
#[cfg(all(test, feature = "standard"))]
pub use builder_cases::run_standard_builder;

#[cfg(all(feature = "fast", feature = "standard"))]
pub(crate) mod differential;
#[cfg(all(test, feature = "fast", feature = "standard"))]
pub use differential::run_differential;
