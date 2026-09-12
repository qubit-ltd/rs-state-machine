// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

/// Offline reachability findings for a validated finite state machine.
///
/// # Type Parameters
/// - `S`: State value returned in each finding.
///
/// # Examples
///
/// ```
/// #[cfg(feature = "standard")]
/// {
///     use qubit_state_machine::StateMachine;
///
///     let machine = StateMachine::<u8, u8>::builder()
///         .add_states(&[0_u8, 1])
///         .initial_state(0)
///         .terminal_state(1)
///         .transition(0, 0, 1)
///         .build()
///         .expect("the graph is valid");
///     let report = machine.diagnose_graph();
///     assert!(report.unreachable_states().is_empty());
/// }
///
/// #[cfg(feature = "fast")]
/// {
///     use qubit_state_machine::FastStateMachine;
///
///     let machine = FastStateMachine::builder()
///         .state_count(2)
///         .event_count(1)
///         .initial_state(0)
///         .terminal_state(1)
///         .transition(0, 0, 1)
///         .build()
///         .expect("the graph is valid");
///     let report = machine.diagnose_graph();
///     assert!(report.unreachable_states().is_empty());
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphDiagnostics<S> {
    /// States not reachable from the configured initial state.
    pub(crate) unreachable_states: Vec<S>,
    /// States with no path to any configured terminal state.
    pub(crate) states_without_terminal_path: Vec<S>,
}

impl<S> GraphDiagnostics<S> {
    /// Returns states that cannot be reached from the initial state.
    ///
    /// # Returns
    /// A slice containing every unreachable state.
    #[must_use]
    #[inline]
    pub fn unreachable_states(&self) -> &[S] {
        &self.unreachable_states
    }

    /// Returns states from which no explicit terminal state can be reached.
    ///
    /// # Returns
    /// A slice containing every state without a terminal path.
    #[must_use]
    #[inline]
    pub fn states_without_terminal_path(&self) -> &[S] {
        &self.states_without_terminal_path
    }
}
