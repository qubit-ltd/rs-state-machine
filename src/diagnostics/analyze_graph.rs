use super::GraphDiagnostics;

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
