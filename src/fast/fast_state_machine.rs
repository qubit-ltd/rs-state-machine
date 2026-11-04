// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Integer-coded state machine implementation.

use qubit_fast_cas::FastCas;
use qubit_fast_cas::FastCasDecision;
use qubit_fast_cas::FastCasPolicy;
use qubit_fast_cas::FastCasState;

use super::FastStateMachineBuilder;
use super::FastStateMachineError;
use super::FastStateMachineResult;
use super::fast_state_machine_error::fast_state_machine_error_from_fast_cas_error;
use crate::GraphDiagnostics;
use crate::Transition;
use crate::diagnostics::analyze_graph;

/// Dense-table sentinel meaning no transition is configured for a cell.
///
/// Valid targets are always in-range state codes; this value marks empty
/// slots during build and is never returned from lookup APIs.
const UNSET_TRANSITION: u64 = u64::MAX;

/// A compact, high-performance state machine backed by [`FastCas`].
///
/// States and events are represented as contiguous integer code spaces:
///
/// - states are in `[0, state_count)`
/// - events are in `[0, event_count)`
///
/// Transition resolution is a single table index lookup:
/// `index = source * event_count + event`.
///
/// # Examples
///
/// ```
/// use qubit_fast_cas::FastCasState;
/// use qubit_state_machine::FastStateMachine;
///
/// let machine = FastStateMachine::builder()
///     .state_count(2)
///     .event_count(1)
///     .initial_state(0)
///     .transition(0, 0, 1)
///     .build()
///     .expect("the rules are valid");
/// let state = FastCasState::new(0);
/// assert_eq!(machine.trigger(&state, 0), Ok(1));
/// ```
#[must_use = "a fast state machine contains the configured transition rules"]
#[derive(Debug, Clone)]
pub struct FastStateMachine {
    /// Number of valid state codes.
    pub(super) state_count: u64,
    /// Number of valid event codes.
    pub(super) event_count: u64,
    /// The unique configured initial state code.
    pub(super) initial_state: u64,
    /// Dense flags indexed by validated state code.
    pub(super) terminal_states: Vec<bool>,
    /// Row-major transition targets, using [`UNSET_TRANSITION`] for empty
    /// cells.
    pub(super) transitions: Vec<u64>,
    /// Number of unique configured transition rules.
    pub(super) transition_count: usize,
    /// Policy-driven CAS executor used by runtime transitions.
    pub(super) cas: FastCas,
}

impl FastStateMachine {
    /// Creates a builder used to configure counts, initial/final flags,
    /// transitions, and CAS policy before calling
    /// [`super::FastStateMachineBuilder::build`].
    ///
    /// # Returns
    /// A new, empty [`super::FastStateMachineBuilder`].
    #[inline]
    pub fn builder() -> FastStateMachineBuilder {
        FastStateMachineBuilder::new()
    }

    /// Creates a new independent fast state cell initialized to the initial
    /// state.
    ///
    /// # Returns
    /// An atomic state cell initialized to [`Self::initial_state`].
    #[must_use]
    #[inline]
    pub fn create_state(&self) -> FastCasState {
        FastCasState::new(self.initial_state)
    }

    /// Returns the number of distinct state codes configured for this machine.
    ///
    /// Valid state codes are integers in `0..state_count()`.
    ///
    /// # Returns
    /// The configured state-space size (length of the transition table rows).
    #[must_use = "use the configured state count"]
    #[inline]
    pub const fn state_count(&self) -> u64 {
        self.state_count
    }

    /// Returns the number of distinct event codes accepted by this machine.
    ///
    /// Valid event codes are integers in `0..event_count()`.
    ///
    /// # Returns
    /// The configured event-space size (length of each row in the transition
    /// table).
    #[must_use]
    #[inline]
    pub const fn event_count(&self) -> u64 {
        self.event_count
    }

    /// Iterates configured transitions in source-code then event-code order.
    ///
    /// # Returns
    /// Only valid transition values; unset dense table cells are omitted.
    ///
    /// # Performance
    /// The iterator is lazy and scans the dense state_count * event_count
    /// table when fully consumed, including unset cells. Full traversal is
    /// O(state_count * event_count); no result collection is allocated.
    #[inline]
    pub fn transitions(&self) -> impl Iterator<Item = Transition<u64, u64>> + '_ {
        (0..self.state_count).flat_map(move |source| {
            (0..self.event_count).filter_map(move |event| {
                self.transition_target(source, event)
                    .map(|target| Transition::new(source, event, target))
            })
        })
    }

    /// Returns the CAS retry policy used for all transitions.
    ///
    /// This is the policy configured in the builder via
    /// [`crate::FastStateMachineBuilder::cas_policy`], or
    /// [`crate::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY`] when no override is
    /// supplied.
    ///
    /// # Returns
    /// The configured Fast CAS policy.
    #[inline]
    pub fn cas_policy(&self) -> FastCasPolicy {
        self.cas.policy()
    }

    /// Returns the unique configured initial state code.
    ///
    /// # Returns
    /// The code used by every new independent state cell.
    #[must_use]
    #[inline]
    pub fn initial_state(&self) -> u64 {
        self.initial_state
    }

    /// Iterates terminal state codes in ascending order.
    ///
    /// # Returns
    /// Codes whose validated rule set forbids outgoing transitions.
    #[inline]
    pub fn terminal_states(&self) -> impl Iterator<Item = u64> + '_ {
        self.terminal_states
            .iter()
            .enumerate()
            .filter_map(|(state, terminal)| terminal.then_some(state as u64))
    }

    /// Returns whether `state` is a valid code for this machine.
    ///
    /// # Parameters
    /// - `state`: Candidate state code.
    ///
    /// # Returns
    /// `true` if `state < state_count()`, otherwise `false`.
    #[must_use]
    #[inline]
    pub const fn contains_state(&self, state: u64) -> bool {
        state < self.state_count
    }

    /// Returns whether `state` was configured as an initial state.
    ///
    /// # Parameters
    /// - `state`: State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked initial; `false` if out of
    /// range or not initial.
    #[must_use]
    #[inline]
    pub fn is_initial_state(&self, state: u64) -> bool {
        state == self.initial_state
    }

    /// Returns whether `state` was configured as a final state.
    ///
    /// # Parameters
    /// - `state`: State code to test.
    ///
    /// # Returns
    /// `true` if `state` is in range and marked final; `false` if out of range
    /// or not final.
    #[must_use]
    #[inline]
    pub fn is_terminal_state(&self, state: u64) -> bool {
        self.state_index(state)
            .and_then(|index| self.terminal_states.get(index))
            .copied()
            .unwrap_or(false)
    }

    /// Returns the number of unique configured transitions.
    ///
    /// # Returns
    /// The count of distinct `(source, event)` pairs with a configured target.
    #[must_use]
    #[inline]
    pub const fn transition_count(&self) -> usize {
        self.transition_count
    }

    /// Looks up the next state for a specific `(source, event)` pair.
    ///
    /// # Parameters
    /// - `source`: Current state code.
    /// - `event`: Event code.
    ///
    /// # Returns
    /// `Some(target)` when a transition is configured; `None` if `source` or
    /// `event` is out of range, or if no transition exists for that pair.
    #[must_use]
    #[inline]
    pub fn transition_target(&self, source: u64, event: u64) -> Option<u64> {
        if !self.contains_state(source) {
            return None;
        }
        self.transition_target_for_valid_state(source, event)
    }

    /// Analyzes reachability and paths to explicit terminal states.
    ///
    /// # Returns
    /// A report listing codes unreachable from the initial state and codes
    /// unable to reach an explicit terminal state.
    ///
    /// # Performance
    /// Each call recomputes the report without caching. It scans the dense
    /// transition table and builds forward and reverse adjacency lists. Time
    /// is O(S * A + S + T), with O(S + T) additional space, where S is the
    /// state count, A the event count, and T the configured transition count.
    /// Use it for offline or infrequent diagnostics.
    #[must_use]
    pub fn diagnose_graph(&self) -> GraphDiagnostics<u64> {
        let index = |code: u64| usize::try_from(code).expect("validated state code fits usize");
        let report = analyze_graph(
            index(self.state_count),
            index(self.initial_state),
            self.terminal_states().map(index),
            self.transitions()
                .map(|edge| (index(edge.source()), index(edge.target()))),
        );
        GraphDiagnostics {
            unreachable_states: report
                .unreachable_states
                .into_iter()
                .map(|index| index as u64)
                .collect(),
            states_without_terminal_path: report
                .states_without_terminal_path
                .into_iter()
                .map(|index| index as u64)
                .collect(),
        }
    }

    /// Applies one event atomically on `state` using the configured [`FastCas`]
    /// policy.
    ///
    /// The cell is not bound to this machine. A conflict retries against the
    /// newly observed state, without detecting intermediate ABA changes.
    /// Reads the current code from `state`, resolves the transition for
    /// `event`, and stores the new code back if the transition is valid.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// `Ok(new_state)` after a successful transition and CAS store.
    ///
    /// # Errors
    /// * [`FastStateMachineError::UnknownState`] — current code not in this
    ///   machine.
    /// * [`FastStateMachineError::UnknownTransition`] — no transition for
    ///   `(current, event)`.
    /// * [`FastStateMachineError::CasConflict`] — CAS retries exhausted.
    #[inline]
    pub fn trigger(&self, state: &FastCasState, event: u64) -> FastStateMachineResult {
        let (_old_state, new_state) = self.change_state(state, event)?;
        Ok(new_state)
    }

    /// Like [`Self::trigger`], but invokes `on_success` after the CAS update
    /// succeeds.
    ///
    /// Callbacks may reenter and execute out of commit order. A callback may
    /// observe a later state; its arguments and this return value describe
    /// only this call's commit. Payload publication requires caller
    /// coordination. The callback receives `(old_state, new_state)` as
    /// observed for the successful transition. It is not called when
    /// [`Self::trigger`] would return an error.
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    /// - `on_success`: Called with previous and new state codes only on
    ///   success.
    ///
    /// # Returns
    /// Same as [`Self::trigger`]: `Ok(new_state)` on success.
    ///
    /// # Errors
    /// Same as [`Self::trigger`].
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    #[inline]
    pub fn trigger_with<F>(&self, state: &FastCasState, event: u64, on_success: F) -> FastStateMachineResult
    where
        F: FnOnce(u64, u64),
    {
        let (old_state, new_state) = self.change_state(state, event)?;
        on_success(old_state, new_state);
        Ok(new_state)
    }

    /// Attempts the same transition as [`Self::trigger`], discarding error
    /// details.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// `true` if the transition was committed; `false` if validation failed or
    /// CAS retries were exhausted. A successful self-transition also returns
    /// `true`.
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline]
    pub fn try_trigger(&self, state: &FastCasState, event: u64) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Like [`Self::trigger_with`], but returns only whether the transition
    /// succeeded.
    ///
    /// `on_success` runs only when the CAS update succeeds, matching
    /// [`Self::trigger_with`].
    ///
    /// # Type Parameters
    /// - `F`: One-shot callback type.
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    /// - `on_success`: Called with previous and new state codes only on
    ///   success.
    ///
    /// # Returns
    /// `true` when the transition commits; `false` on any error that
    /// [`Self::trigger_with`] would surface.
    ///
    /// # Panics
    /// A panic from `on_success` propagates after the state transition has
    /// already been committed.
    #[must_use = "the boolean result reports whether the transition committed"]
    #[inline]
    pub fn try_trigger_with<F>(&self, state: &FastCasState, event: u64, on_success: F) -> bool
    where
        F: FnOnce(u64, u64),
    {
        self.trigger_with(state, event, on_success).is_ok()
    }

    /// Resolves a transition after the caller has validated the source state.
    ///
    /// # Parameters
    /// - `source`: Source state known to be less than [`Self::state_count`].
    /// - `event`: Event code to resolve.
    ///
    /// # Returns
    /// The configured target, or `None` when `event` is out of range or the
    /// transition cell is unset.
    #[must_use]
    #[inline]
    fn transition_target_for_valid_state(&self, source: u64, event: u64) -> Option<u64> {
        let index = self.transition_index(source, event)?;
        self.transitions
            .get(index)
            .copied()
            .filter(|&target| target != UNSET_TRANSITION)
    }

    /// Runs one CAS-backed transition: validates `event` against the loaded
    /// current code and installs the next code or aborts with
    /// [`FastStateMachineError`].
    ///
    /// # Parameters
    /// - `state`: Shared compact state updated by compare-and-swap.
    /// - `event`: Event code to apply.
    ///
    /// # Returns
    /// The previous and committed state codes.
    ///
    /// # Errors
    /// Returns an unknown-state, unknown-transition, or exhausted-conflict
    /// error.
    fn change_state(&self, state: &FastCasState, event: u64) -> Result<(u64, u64), FastStateMachineError> {
        match self.cas.execute::<u64, FastStateMachineError, _>(state, |current| {
            #[cfg(test)]
            retry_interleaving_tests::before_decision(current);
            match self.next_state(current, event) {
                Ok(new_state) => FastCasDecision::update(new_state, new_state),
                Err(error) => FastCasDecision::abort(error),
            }
        }) {
            Ok(success) => Ok((success.previous(), success.current())),
            Err(error) => Err(fast_state_machine_error_from_fast_cas_error(error)),
        }
    }

    /// Validates `state` and resolves the successor for `event` using the
    /// transition table.
    ///
    /// # Parameters
    /// - `state`: Current state code.
    /// - `event`: Event code to resolve.
    ///
    /// # Returns
    /// The configured target state.
    ///
    /// # Errors
    /// Returns [`FastStateMachineError::UnknownState`] for an out-of-range
    /// state, or [`FastStateMachineError::UnknownTransition`] when no target is
    /// configured for the pair.
    #[inline]
    fn next_state(&self, state: u64, event: u64) -> Result<u64, FastStateMachineError> {
        if !self.contains_state(state) {
            return Err(FastStateMachineError::UnknownState { state });
        }

        self.transition_target_for_valid_state(state, event)
            .ok_or(FastStateMachineError::UnknownTransition {
                source_state: state,
                event,
            })
    }

    /// Converts an in-range state code into a slice index.
    ///
    /// # Parameters
    /// - `state`: Candidate state code.
    ///
    /// # Returns
    /// The platform index for an in-range code, or `None` otherwise.
    #[must_use]
    #[inline]
    fn state_index(&self, state: u64) -> Option<usize> {
        if !self.contains_state(state) {
            return None;
        }
        usize::try_from(state).ok()
    }

    /// Computes the row-major table index for a validated state and event pair.
    ///
    /// # Parameters
    /// - `source`: Source state already known to be in range.
    /// - `event`: Candidate event code.
    ///
    /// # Returns
    /// The platform table index, or `None` when `event` or the computed index
    /// is not representable.
    #[must_use]
    #[inline]
    fn transition_index(&self, source: u64, event: u64) -> Option<usize> {
        if event >= self.event_count {
            return None;
        }
        let index = source.checked_mul(self.event_count)?.checked_add(event)?;
        usize::try_from(index).ok()
    }
}

#[cfg(test)]
mod retry_interleaving_tests {
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::thread::spawn;
    use std::time::Duration;

    use qubit_fast_cas::FastCasPolicy;

    use super::super::typed::DenseCode;
    use super::super::typed::TypedFastStateMachine;
    use super::super::typed::TypedFastStateMachineError;
    use super::FastStateMachine;
    use super::FastStateMachineError;

    const CHANNEL_TIMEOUT: Duration = Duration::from_secs(5);
    const A: u64 = 0;
    const B: u64 = 1;
    const C: u64 = 2;
    const D: u64 = 3;
    const WORK: u64 = 0;
    const ADVANCE: u64 = 1;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum State {
        A,
        B,
        C,
        D,
    }

    impl DenseCode for State {
        const VALUES: &'static [Self] = &[Self::A, Self::B, Self::C, Self::D];

        fn code(self) -> u64 {
            match self {
                Self::A => A,
                Self::B => B,
                Self::C => C,
                Self::D => D,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Event {
        Work,
        Advance,
    }

    impl DenseCode for Event {
        const VALUES: &'static [Self] = &[Self::Work, Self::Advance];

        fn code(self) -> u64 {
            match self {
                Self::Work => WORK,
                Self::Advance => ADVANCE,
            }
        }
    }

    /// Coordinates a worker pause after loading a code but before deciding a
    /// CAS update.
    struct Gate {
        pause_decisions: usize,
        decisions: usize,
        observed: Vec<u64>,
        failed: bool,
        entered: mpsc::Sender<u64>,
        release: mpsc::Receiver<()>,
    }

    thread_local! {
        static GATE: RefCell<Option<Gate>> = const { RefCell::new(None) };
    }

    /// Clears the thread-local gate even when a worker unwinds.
    struct GateGuard;

    impl GateGuard {
        /// Installs the worker-local gate, rejecting nested installations.
        fn install(gate: Gate) -> Self {
            GATE.with(|slot| {
                let previous = slot.replace(Some(gate));
                assert!(previous.is_none(), "retry gate already installed on worker");
            });
            Self
        }

        /// Copies observed decision codes and the coordination failure flag.
        fn take_result(&self) -> (Vec<u64>, bool) {
            GATE.with(|slot| {
                let slot = slot.borrow_mut();
                let gate = slot.as_ref().expect("retry gate is installed");
                (gate.observed.clone(), gate.failed)
            })
        }
    }

    impl Drop for GateGuard {
        fn drop(&mut self) {
            GATE.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }

    /// Pauses selected decisions until the coordinator commits a competing
    /// transition.
    pub(super) fn before_decision(current: u64) {
        GATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(gate) = slot.as_mut() else {
                return;
            };
            gate.decisions += 1;
            gate.observed.push(current);
            if gate.decisions <= gate.pause_decisions
                && (gate.entered.send(current).is_err() || gate.release.recv_timeout(CHANNEL_TIMEOUT).is_err())
            {
                gate.failed = true;
            }
        });
    }

    /// Builds integer rules for invalidation, retargeting, or budget
    /// exhaustion.
    fn raw_machine(policy: FastCasPolicy, retry_work: bool, exhaust_work: bool) -> FastStateMachine {
        let mut builder = FastStateMachine::builder()
            .state_count(4)
            .event_count(2)
            .initial_state(A)
            .transition(A, WORK, if exhaust_work { D } else { C })
            .transition(A, ADVANCE, B)
            .cas_policy(policy);
        if retry_work || exhaust_work {
            builder = builder.transition(B, WORK, D);
        }
        if exhaust_work {
            builder = builder.transition(B, ADVANCE, C).transition(C, WORK, D);
        }
        builder.build().expect("test transition rules are valid")
    }

    /// Builds typed equivalents of the controlled integer transition rules.
    fn typed_machine(
        policy: FastCasPolicy,
        retry_work: bool,
        exhaust_work: bool,
    ) -> TypedFastStateMachine<State, Event> {
        let mut builder = TypedFastStateMachine::<State, Event>::builder()
            .initial_state(State::A)
            .transition(State::A, Event::Work, if exhaust_work { State::D } else { State::C })
            .transition(State::A, Event::Advance, State::B)
            .cas_policy(policy);
        if retry_work || exhaust_work {
            builder = builder.transition(State::B, Event::Work, State::D);
        }
        if exhaust_work {
            builder =
                builder
                    .transition(State::B, Event::Advance, State::C)
                    .transition(State::C, Event::Work, State::D);
        }
        builder.build().expect("test transition rules are valid")
    }

    /// Captures the operation result, callback commits, decision observations,
    /// and final cell.
    struct WorkerResult<R> {
        result: R,
        callback_args: Vec<(u64, u64)>,
        observed: Vec<u64>,
        final_state: u64,
    }

    /// Runs integer transitions with deterministic pauses before selected CAS
    /// decisions.
    fn run_raw(
        machine: Arc<FastStateMachine>,
        pause_decisions: usize,
        advances: usize,
    ) -> WorkerResult<Result<u64, FastStateMachineError>> {
        let state = Arc::new(machine.create_state());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker_machine = Arc::clone(&machine);
        let worker_state = Arc::clone(&state);
        let worker = spawn(move || {
            let guard = GateGuard::install(Gate {
                pause_decisions,
                decisions: 0,
                observed: Vec::new(),
                failed: false,
                entered: entered_tx,
                release: release_rx,
            });
            let mut callback_args = Vec::new();
            let result = worker_machine.trigger_with(&worker_state, WORK, |old, new| callback_args.push((old, new)));
            let (observed, gate_failed) = guard.take_result();
            let _ = result_tx.send((result, callback_args, observed, gate_failed));
        });

        let mut coordination_error = None;
        for index in 0..advances {
            let observed = match entered_rx.recv_timeout(CHANNEL_TIMEOUT) {
                Ok(observed) => observed,
                Err(error) => {
                    coordination_error = Some(format!("worker did not reach decision point: {error}"));
                    break;
                }
            };
            if observed != A + index as u64 {
                coordination_error = Some(format!("worker read unexpected state {observed}"));
            }
            let expected = A + index as u64 + 1;
            if machine.trigger(&state, ADVANCE) != Ok(expected) {
                coordination_error = Some(format!("competing transition did not reach {expected}"));
            }
            if release_tx.send(()).is_err() {
                coordination_error = Some("worker left its decision point before release".to_owned());
                break;
            }
        }
        drop(release_tx);
        let received = result_rx.recv_timeout(CHANNEL_TIMEOUT);
        let joined = worker.join();
        assert!(
            coordination_error.is_none(),
            "controlled interleaving failed: {coordination_error:?}"
        );
        assert!(joined.is_ok(), "worker thread completes without panicking");
        let (result, callback_args, observed, gate_failed) = received.expect("worker reports target result");
        assert!(!gate_failed, "worker gate did not time out or lose its coordinator");
        WorkerResult {
            result,
            callback_args,
            observed,
            final_state: state.load(),
        }
    }

    /// Runs typed transitions through the same private integer decision gate.
    fn run_typed(
        machine: Arc<TypedFastStateMachine<State, Event>>,
        pause_decisions: usize,
        advances: usize,
    ) -> WorkerResult<Result<State, TypedFastStateMachineError<State, Event>>> {
        let state = Arc::new(machine.create_state());
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let worker_machine = Arc::clone(&machine);
        let worker_state = Arc::clone(&state);
        let worker = spawn(move || {
            let guard = GateGuard::install(Gate {
                pause_decisions,
                decisions: 0,
                observed: Vec::new(),
                failed: false,
                entered: entered_tx,
                release: release_rx,
            });
            let mut callback_args = Vec::new();
            let result = worker_machine.trigger_with(&worker_state, Event::Work, |old, new| {
                callback_args.push((old.code(), new.code()));
            });
            let (observed, gate_failed) = guard.take_result();
            let _ = result_tx.send((result, callback_args, observed, gate_failed));
        });

        let mut coordination_error = None;
        for index in 0..advances {
            let observed = match entered_rx.recv_timeout(CHANNEL_TIMEOUT) {
                Ok(observed) => observed,
                Err(error) => {
                    coordination_error = Some(format!("worker did not reach decision point: {error}"));
                    break;
                }
            };
            if observed != A + index as u64 {
                coordination_error = Some(format!("worker read unexpected state {observed}"));
            }
            let expected = [State::B, State::C][index];
            if machine.trigger(&state, Event::Advance) != Ok(expected) {
                coordination_error = Some(format!("competing transition did not reach {expected:?}"));
            }
            if release_tx.send(()).is_err() {
                coordination_error = Some("worker left its decision point before release".to_owned());
                break;
            }
        }
        drop(release_tx);
        let received = result_rx.recv_timeout(CHANNEL_TIMEOUT);
        let joined = worker.join();
        assert!(
            coordination_error.is_none(),
            "controlled interleaving failed: {coordination_error:?}"
        );
        assert!(joined.is_ok(), "worker thread completes without panicking");
        let (result, callback_args, observed, gate_failed) = received.expect("worker reports target result");
        assert!(!gate_failed, "worker gate did not time out or lose its coordinator");
        WorkerResult {
            result,
            callback_args,
            observed,
            final_state: state.load().code(),
        }
    }

    #[test]
    fn test_raw_fast_retry_rejects_invalidated_event() {
        let machine = Arc::new(raw_machine(FastCasPolicy::spin(2), false, false));
        let result = run_raw(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(FastStateMachineError::UnknownTransition {
                source_state: B,
                event: WORK
            }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, B,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_typed_fast_retry_rejects_invalidated_event() {
        let machine = Arc::new(typed_machine(FastCasPolicy::spin(2), false, false));
        let result = run_typed(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(TypedFastStateMachineError::UnknownTransition {
                source_state: State::B,
                event: Event::Work
            }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, B,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_raw_fast_retry_recomputes_target() {
        let machine = Arc::new(raw_machine(FastCasPolicy::spin(2), true, false));
        let result = run_raw(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Ok(D),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [(B, D)],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, D,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_typed_fast_retry_recomputes_target() {
        let machine = Arc::new(typed_machine(FastCasPolicy::spin(2), true, false));
        let result = run_typed(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Ok(State::D),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [(B, D)],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, D,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_raw_fast_single_attempt_conflict() {
        let machine = Arc::new(raw_machine(FastCasPolicy::once(), true, false));
        let result = run_raw(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(FastStateMachineError::CasConflict { attempts: 1 }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, B,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_typed_fast_single_attempt_conflict() {
        let machine = Arc::new(typed_machine(FastCasPolicy::once(), true, false));
        let result = run_typed(machine, 1, 1);
        assert_eq!(
            result.observed,
            [A],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(TypedFastStateMachineError::CasConflict { attempts: 1 }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, B,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_raw_fast_retry_budget_exhausted() {
        let machine = Arc::new(raw_machine(FastCasPolicy::spin(2), false, true));
        let result = run_raw(machine, 2, 2);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(FastStateMachineError::CasConflict { attempts: 2 }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, C,
            "the shared cell must retain the expected final state"
        );
    }

    #[test]
    fn test_typed_fast_retry_budget_exhausted() {
        let machine = Arc::new(typed_machine(FastCasPolicy::spin(2), false, true));
        let result = run_typed(machine, 2, 2);
        assert_eq!(
            result.observed,
            [A, B],
            "decisions must observe the expected retry state codes"
        );
        assert_eq!(
            result.result,
            Err(TypedFastStateMachineError::CasConflict { attempts: 2 }),
            "the controlled interleaving must produce the expected transition result"
        );
        assert_eq!(
            result.callback_args,
            [],
            "callbacks must match only the successfully committed transition"
        );
        assert_eq!(
            result.final_state, C,
            "the shared cell must retain the expected final state"
        );
    }
}
