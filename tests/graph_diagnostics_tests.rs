// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "fast")]
#[test]
fn fast_graph_diagnostics_report_unreachable_and_nonterminal_states() {
    use qubit_state_machine::FastStateMachine;

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

#[cfg(feature = "standard")]
#[test]
fn standard_graph_diagnostics_report_unreachable_and_nonterminal_states() {
    use qubit_state_machine::StateMachine;

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
