// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Reachability analysis helpers shared by standard and fast state machines.

mod analyze_graph;
mod graph_diagnostics;

pub(crate) use analyze_graph::analyze_graph;
pub use graph_diagnostics::GraphDiagnostics;
