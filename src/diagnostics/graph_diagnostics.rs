/// Offline reachability findings for a validated finite state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphDiagnostics<S> {
    pub(crate) unreachable_states: Vec<S>,
    pub(crate) states_without_terminal_path: Vec<S>,
}

impl<S> GraphDiagnostics<S> {
    /// Returns states that cannot be reached from the initial state.
    #[must_use]
    pub fn unreachable_states(&self) -> &[S] {
        &self.unreachable_states
    }

    /// Returns states from which no explicit terminal state can be reached.
    #[must_use]
    pub fn states_without_terminal_path(&self) -> &[S] {
        &self.states_without_terminal_path
    }
}
