// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Integration tests for `qubit-state-machine`.

#[cfg(feature = "fast")]
mod fast;
#[cfg(feature = "standard")]
mod standard;
