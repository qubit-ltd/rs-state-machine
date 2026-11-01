// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "fast")]
use qubit_state_machine::FastStateMachine;
#[cfg(feature = "standard")]
use qubit_state_machine::StateMachine;

#[cfg(any(feature = "standard", feature = "fast"))]
struct GraphCase {
    name: &'static str,
    edges: &'static [(u64, u64, u64)],
    terminals: &'static [u64],
    unreachable: &'static [u64],
    without_path: &'static [u64],
}

#[cfg(any(feature = "standard", feature = "fast"))]
const GRAPH_CASES: [GraphCase; 4] = [
    GraphCase {
        name: "no terminals",
        edges: &[],
        terminals: &[],
        unreachable: &[1, 2],
        without_path: &[0, 1, 2],
    },
    GraphCase {
        name: "initial is terminal",
        edges: &[],
        terminals: &[0],
        unreachable: &[1, 2],
        without_path: &[1, 2],
    },
    GraphCase {
        name: "closed cycle",
        edges: &[(0, 0, 1), (1, 0, 0)],
        terminals: &[2],
        unreachable: &[2],
        without_path: &[0, 1],
    },
    GraphCase {
        name: "cycle with exit",
        edges: &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
        terminals: &[2],
        unreachable: &[],
        without_path: &[],
    },
];

#[cfg(feature = "fast")]
#[test]
fn test_fast_graph_diagnostics_report_unreachable_and_nonterminal_states() {
    let machine = FastStateMachine::builder()
        .state_count(5)
        .event_count(2)
        .initial_state(0)
        .terminal_state(2)
        .transition(0, 0, 1)
        .transition(1, 0, 2)
        .transition(0, 1, 3)
        .transition(3, 0, 3)
        .build()
        .expect("valid graph");
    let report = machine.diagnose_graph();
    assert_eq!(report.unreachable_states(), &[4]);
    assert_eq!(report.states_without_terminal_path(), &[3, 4]);
}

#[cfg(feature = "fast")]
#[test]
fn test_fast_graph_diagnostics_boundary_cases() {
    for case in &GRAPH_CASES {
        let mut builder = FastStateMachine::builder()
            .state_count(3)
            .event_count(2)
            .initial_state(0)
            .terminal_states(case.terminals);
        for &(source, event, target) in case.edges {
            builder = builder.transition(source, event, target);
        }
        let machine = builder.build().expect(case.name);
        let report = machine.diagnose_graph();
        assert_eq!(report.unreachable_states(), case.unreachable, "{}", case.name);
        assert_eq!(
            report.states_without_terminal_path(),
            case.without_path,
            "{}",
            case.name
        );
    }
}

#[cfg(feature = "standard")]
#[test]
fn test_standard_graph_diagnostics_report_unreachable_and_nonterminal_states() {
    let machine = StateMachine::builder()
        .add_states(&[0_u8, 1, 2, 3, 4])
        .initial_state(0)
        .terminal_state(2)
        .transition(0, 0, 1)
        .transition(1, 0, 2)
        .transition(0, 1, 3)
        .transition(3, 0, 3)
        .build()
        .expect("valid graph");
    let report = machine.diagnose_graph();
    let mut unreachable = report.unreachable_states().to_vec();
    unreachable.sort_unstable();
    let mut without_path = report.states_without_terminal_path().to_vec();
    without_path.sort_unstable();
    assert_eq!(unreachable, vec![4]);
    assert_eq!(without_path, vec![3, 4]);
}

#[cfg(feature = "standard")]
#[test]
fn test_standard_graph_diagnostics_boundary_cases() {
    for case in &GRAPH_CASES {
        let mut builder = StateMachine::builder()
            .add_states(&[0_u64, 1, 2])
            .initial_state(0)
            .terminal_states(case.terminals);
        for &(source, event, target) in case.edges {
            builder = builder.transition(source, event, target);
        }
        let machine = builder.build().expect(case.name);
        let report = machine.diagnose_graph();
        let mut unreachable = report.unreachable_states().to_vec();
        let mut without_path = report.states_without_terminal_path().to_vec();
        unreachable.sort_unstable();
        without_path.sort_unstable();
        assert_eq!(unreachable.as_slice(), case.unreachable, "{}", case.name);
        assert_eq!(without_path.as_slice(), case.without_path, "{}", case.name);
    }
}
