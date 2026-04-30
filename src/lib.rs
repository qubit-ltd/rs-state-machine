/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! # Qubit State Machine
//!
//! A small, thread-safe finite state machine for Rust.
//!
//! This crate is intentionally compact. It stores immutable transition rules
//! and applies events to an [`AtomicRef`] through a compare-and-swap executor.
//!
//! # Examples
//!
//! ```
//! use qubit_state_machine::{AtomicRef, StateMachine};
//!
//! #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
//! enum State {
//!     New,
//!     Running,
//!     Done,
//! }
//!
//! #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
//! enum Event {
//!     Start,
//!     Finish,
//! }
//!
//! let mut builder = StateMachine::builder();
//! builder.add_states(&[State::New, State::Running, State::Done]);
//! builder.set_initial_state(State::New);
//! builder.set_final_state(State::Done);
//! builder.add_transition(State::New, Event::Start, State::Running);
//! builder.add_transition(State::Running, Event::Finish, State::Done);
//! let machine = builder.build().expect("state machine should be valid");
//!
//! let state = AtomicRef::from_value(State::New);
//! assert_eq!(machine.trigger(&state, Event::Start).unwrap(), State::Running);
//! assert_eq!(*state.load(), State::Running);
//! ```
//!
//! # Author
//!
//! Haixing Hu

#![deny(missing_docs)]

mod state_machine;
mod state_machine_build_error;
mod state_machine_builder;
mod state_machine_error;
mod transition;

pub use qubit_atomic::AtomicRef;
pub use state_machine::StateMachine;
pub use state_machine_build_error::StateMachineBuildError;
pub use state_machine_builder::StateMachineBuilder;
pub use state_machine_error::StateMachineError;
pub use transition::Transition;

/// Result returned by event-triggering state machine operations.
///
/// `S` is the state type and `E` is the event type.
pub type StateMachineResult<S, E> = Result<S, StateMachineError<S, E>>;
