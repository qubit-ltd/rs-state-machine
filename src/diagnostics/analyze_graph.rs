// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Internal graph algorithms backing [`super::GraphDiagnostics`].

use super::GraphDiagnostics;

/// Marks every vertex reachable from the supplied seeds.
///
/// # Parameters
/// - `adjacency`: Directed adjacency lists indexed by vertex.
/// - `seeds`: In-range vertices from which traversal starts.
///
/// # Returns
/// A boolean vector with one entry per vertex; entries are `true` exactly for
/// vertices reachable from a seed.
///
/// # Panics
/// Panics if a seed or a reachable edge target is outside `adjacency`.
#[must_use]
fn mark_reachable(adjacency: &[Vec<usize>], seeds: impl IntoIterator<Item = usize>) -> Vec<bool> {
    let mut seen = vec![false; adjacency.len()];
    let mut stack = Vec::new();
    for seed in seeds {
        if !seen[seed] {
            seen[seed] = true;
            stack.push(seed);
        }
    }
    while let Some(source) = stack.pop() {
        for &target in &adjacency[source] {
            if !seen[target] {
                seen[target] = true;
                stack.push(target);
            }
        }
    }
    seen
}

/// Computes reachability findings for a finite directed state graph.
///
/// # Parameters
/// - `state_count`: Number of vertices; all supplied indices must be below it.
/// - `initial`: In-range initial vertex used for forward reachability.
/// - `terminals`: In-range vertices considered explicit terminal states.
/// - `edges`: Directed transitions whose source and target are in range.
///
/// # Returns
/// A report containing unreachable states and states with no path to a
/// terminal.
///
/// The caller must provide a nonempty graph and valid vertex indices. The
/// function performs no validation before indexing the adjacency lists.
///
/// # Panics
/// Panics if `initial`, a terminal, or an edge endpoint is at least
/// `state_count`, including when `state_count` is zero.
#[must_use]
pub(crate) fn analyze_graph(
    state_count: usize,
    initial: usize,
    terminals: impl IntoIterator<Item = usize>,
    edges: impl IntoIterator<Item = (usize, usize)>,
) -> GraphDiagnostics<usize> {
    let mut forward = vec![Vec::new(); state_count];
    let mut reverse = vec![Vec::new(); state_count];
    for (source, target) in edges {
        forward[source].push(target);
        reverse[target].push(source);
    }
    let from_initial = mark_reachable(&forward, [initial]);
    let to_terminal = mark_reachable(&reverse, terminals);
    GraphDiagnostics {
        unreachable_states: (0..state_count).filter(|&i| !from_initial[i]).collect(),
        states_without_terminal_path: (0..state_count).filter(|&i| !to_terminal[i]).collect(),
    }
}
