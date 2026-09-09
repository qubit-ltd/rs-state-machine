// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Type-checked entry points over the existing immutable Fast engine.
use std::marker::PhantomData;

use qubit_fast_cas::FastCasPolicy;

use super::DenseCode;
use super::TypedFastState;
use super::TypedFastStateMachineBuilder;
use super::TypedFastStateMachineError;
use super::dense_code::checked_code;
use super::dense_code::decode;
use crate::FastStateMachine;
use crate::Transition;

/// Immutable dense rules for finite state `S` and event `E` types.
///
/// Storage and CAS retries are delegated to [`FastStateMachine`]. Typed cells
/// have no machine identity and may be used by another machine with the same
/// state type. Only the numeric state commits atomically: callbacks and payload
/// publication are separate. Retry recomputes the event from the observed
/// state; intermediate ABA changes and callback ordering are not detected or
/// enforced.
///
/// # Typed inputs
///
/// ```
/// use qubit_state_machine::{DenseCode, TypedFastStateMachine};
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Event { Tick }
/// impl DenseCode for Event {
///     const VALUES: &'static [Self] = &[Self::Tick];
///     fn code(self) -> u64 { 0 }
/// }
/// let machine = TypedFastStateMachine::<State, Event>::builder()
///     .initial_state(State::Ready).transition(State::Ready, Event::Tick, State::Ready)
///     .build().expect("valid typed machine");
/// let state = machine.create_state();
/// assert_eq!(machine.trigger(&state, Event::Tick).expect("commit"), State::Ready);
/// ```
///
/// State and event types cannot be interchanged:
///
/// ```compile_fail,E0308
/// use qubit_state_machine::{DenseCode, TypedFastStateMachine};
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Event { Tick }
/// impl DenseCode for Event {
///     const VALUES: &'static [Self] = &[Self::Tick];
///     fn code(self) -> u64 { 0 }
/// }
/// let machine = TypedFastStateMachine::<State, Event>::builder()
///     .initial_state(State::Ready).transition(State::Ready, Event::Tick, State::Ready)
///     .build().expect("valid typed machine");
/// let state = machine.create_state();
/// let _result = machine.trigger(&state, State::Ready);
/// ```
///
/// A cell with a different state type cannot be passed to this machine:
///
/// ```compile_fail,E0308
/// use qubit_state_machine::{DenseCode, TypedFastStateMachine};
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum State { Ready }
/// impl DenseCode for State {
///     const VALUES: &'static [Self] = &[Self::Ready];
///     fn code(self) -> u64 { 0 }
/// }
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Event { Tick }
/// impl DenseCode for Event {
///     const VALUES: &'static [Self] = &[Self::Tick];
///     fn code(self) -> u64 { 0 }
/// }
/// let machine = TypedFastStateMachine::<State, Event>::builder()
///     .initial_state(State::Ready).transition(State::Ready, Event::Tick, State::Ready)
///     .build().expect("valid typed machine");
/// let state = machine.create_state();
/// let other = TypedFastStateMachine::<Event, Event>::builder()
///     .initial_state(Event::Tick).build().expect("valid second machine");
/// let _result = machine.trigger(&other.create_state(), Event::Tick);
/// ```
#[must_use = "a typed state machine contains the configured transition rules"]
#[derive(Debug, Clone)]
pub struct TypedFastStateMachine<S: DenseCode, E: DenseCode> {
    /// Shared dense lookup and CAS implementation.
    pub(super) raw: FastStateMachine,
    /// State and event type markers with no runtime storage.
    pub(super) marker: PhantomData<fn() -> (S, E)>,
}

impl<S: DenseCode, E: DenseCode> TypedFastStateMachine<S, E> {
    /// Creates an empty typed builder with inferred state and event counts.
    pub fn builder() -> TypedFastStateMachineBuilder<S, E> {
        TypedFastStateMachineBuilder::new()
    }

    /// Creates an independent cell containing the configured initial state.
    #[must_use = "use the independent state cell"]
    pub fn create_state(&self) -> TypedFastState<S> {
        TypedFastState {
            raw: self.raw.create_state(),
            marker: PhantomData,
        }
    }

    /// Applies `event` to the shared typed `state` through the configured CAS
    /// policy.
    ///
    /// # Returns
    /// This call's committed target, which may already be superseded by another
    /// caller.
    ///
    /// # Errors
    /// Returns invalid-event membership, unknown-transition, or CAS-budget
    /// errors.
    ///
    /// # Panics
    /// Panics if an internal committed code violates the validated codebook.
    #[inline]
    pub fn trigger(&self, state: &TypedFastState<S>, event: E) -> Result<S, TypedFastStateMachineError> {
        let code = checked_code(event).ok_or(TypedFastStateMachineError::InvalidEventCode { code: event.code() })?;
        let next = self.raw.trigger(&state.raw, code)?;
        Ok(decode(next).expect("validated transition target must decode"))
    }

    /// Applies `event` to `state` and calls `on_success` once after committing.
    ///
    /// The callback receives this commit's old and new states. It can reenter
    /// the machine; another callback may run first or observe a later state.
    ///
    /// # Type Parameters
    /// - `F`: A one-shot callback, executed outside the retry loop.
    ///
    /// # Returns
    /// This call's target, even if a reentrant callback commits another state.
    ///
    /// # Errors
    /// Returns the same errors as `trigger`; errors never invoke the callback.
    ///
    /// # Panics
    /// Callback panics propagate after the commit, without rolling it back.
    /// An internal codebook violation also panics safely.
    #[inline]
    pub fn trigger_with<F>(
        &self,
        state: &TypedFastState<S>,
        event: E,
        on_success: F,
    ) -> Result<S, TypedFastStateMachineError>
    where
        F: FnOnce(S, S),
    {
        let code = checked_code(event).ok_or(TypedFastStateMachineError::InvalidEventCode { code: event.code() })?;
        let next = self.raw.trigger_with(&state.raw, code, |old, new| {
            on_success(
                decode(old).expect("validated previous state must decode"),
                decode(new).expect("validated transition target must decode"),
            );
        })?;
        Ok(decode(next).expect("validated transition target must decode"))
    }

    /// Applies `event` to `state`, discarding every error classification.
    ///
    /// # Returns
    /// `true` on commit (including self-transitions), or `false` for any error.
    #[must_use = "the result reports whether the transition committed"]
    #[inline]
    pub fn try_trigger(&self, state: &TypedFastState<S>, event: E) -> bool {
        self.trigger(state, event).is_ok()
    }

    /// Applies `event` to `state`, calling `callback` only on a successful
    /// commit.
    ///
    /// # Returns
    /// `true` on commit, or `false` on any validation or CAS error.
    ///
    /// # Panics
    /// Propagates callback panics without rolling back the committed state.
    #[must_use = "the result reports whether the transition committed"]
    #[inline]
    pub fn try_trigger_with<F>(&self, state: &TypedFastState<S>, event: E, callback: F) -> bool
    where
        F: FnOnce(S, S),
    {
        self.trigger_with(state, event, callback).is_ok()
    }

    /// Queries the target for `source` and `event` without modifying a cell.
    ///
    /// # Returns
    /// The target, or `None` for an invalid typed input or an undefined
    /// transition.
    #[inline]
    pub fn transition_target(&self, source: S, event: E) -> Option<S> {
        decode(
            self.raw
                .transition_target(checked_code(source)?, checked_code(event)?)?,
        )
    }

    /// Returns the unique configured initial state.
    ///
    /// # Panics
    /// Panics if the raw machine violates its validated initial-code invariant.
    pub fn initial_state(&self) -> S {
        decode(self.raw.initial_state()).expect("validated initial state must decode")
    }

    /// Returns whether `state` belongs to its declared finite value table.
    pub fn contains_state(&self, state: S) -> bool {
        checked_code(state).is_some()
    }

    /// Returns whether `state` is valid and is the initial state.
    pub fn is_initial_state(&self, state: S) -> bool {
        checked_code(state).is_some_and(|code| self.raw.is_initial_state(code))
    }

    /// Returns whether `state` is valid and marked terminal.
    pub fn is_terminal_state(&self, state: S) -> bool {
        checked_code(state).is_some_and(|code| self.raw.is_terminal_state(code))
    }

    /// Returns the number of states in the finite codebook.
    pub const fn state_count(&self) -> u64 {
        self.raw.state_count()
    }

    /// Returns the number of events in the finite codebook.
    pub const fn event_count(&self) -> u64 {
        self.raw.event_count()
    }

    /// Returns the number of distinct configured transition rules.
    pub const fn transition_count(&self) -> usize {
        self.raw.transition_count()
    }

    /// Returns the policy used for each CAS transition.
    pub fn cas_policy(&self) -> FastCasPolicy {
        self.raw.cas_policy()
    }

    /// Iterates terminal values in ascending code order.
    ///
    /// # Panics
    /// Iteration panics if an internal terminal code cannot be decoded.
    pub fn terminal_states(&self) -> impl Iterator<Item = S> + '_ {
        self.raw
            .terminal_states()
            .map(|code| decode(code).expect("validated terminal state must decode"))
    }

    /// Iterates configured rules in source-code then event-code order.
    ///
    /// # Panics
    /// Iteration panics if an internal rule contains an invalid code.
    pub fn transitions(&self) -> impl Iterator<Item = Transition<S, E>> + '_ {
        self.raw.transitions().map(|value| {
            Transition::new(
                decode(value.source()).expect("validated transition source must decode"),
                decode(value.event()).expect("validated transition event must decode"),
                decode(value.target()).expect("validated transition target must decode"),
            )
        })
    }
}
