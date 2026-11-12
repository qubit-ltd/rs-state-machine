// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

/// Offline reachability findings for a validated finite state machine.
///
/// If no explicit terminal state is configured, every registered state is
/// reported as having no terminal path. This is diagnostic information and
/// does not invalidate the machine.
///
/// Finding order depends on the machine implementation: `StateMachine`
/// results have no guaranteed stable order, while `FastStateMachine` and
/// `TypedFastStateMachine` results are ordered by ascending state code. The
/// derived [`PartialEq`] and [`Eq`] implementations compare both finding
/// vectors element by element, including their order; they do not compare the
/// findings as sets. Sort findings in the caller when order-independent
/// comparisons are needed.
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
/// #[cfg(feature = "standard")]
/// {
///     use qubit_state_machine::StateMachine;
///
///     let build = || StateMachine::<u8, u8>::builder()
///         .add_states(&[0, 1, 2, 3])
///         .initial_state(0)
///         .terminal_state(1)
///         .transition(0, 0, 1)
///         .build()
///         .expect("valid graph");
///     let left = build().diagnose_graph();
///     let right = build().diagnose_graph();
///     // Standard-machine finding order is not stable; sort in the caller
///     // before comparing reports independently of their discovery order.
///     let sorted = |values: &[u8]| {
///         let mut values = values.to_vec();
///         values.sort_unstable();
///         values
///     };
///     assert_eq!(sorted(left.unreachable_states()), vec![2, 3]);
///     assert_eq!(
///         sorted(left.unreachable_states()),
///         sorted(right.unreachable_states())
///     );
///     assert_eq!(sorted(left.states_without_terminal_path()), vec![2, 3]);
///     assert_eq!(
///         sorted(left.states_without_terminal_path()),
///         sorted(right.states_without_terminal_path())
///     );
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
    /// The order follows the machine implementation: it is not guaranteed to
    /// be stable for `StateMachine` and is ascending by state code for
    /// `FastStateMachine` and `TypedFastStateMachine`.
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
    /// The order follows the machine implementation: it is not guaranteed to
    /// be stable for `StateMachine` and is ascending by state code for
    /// `FastStateMachine` and `TypedFastStateMachine`.
    ///
    /// # Returns
    /// A slice containing every state without a terminal path.
    #[must_use]
    #[inline]
    pub fn states_without_terminal_path(&self) -> &[S] {
        &self.states_without_terminal_path
    }
}
