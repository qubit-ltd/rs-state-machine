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
//! and applies events to a [`StateCell`] so each state change is guarded by a
//! single critical section.
//!
//! # Examples
//!
//! ```
//! use qubit_state_machine::{StateCell, StateMachine};
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
//! let state = StateCell::new(State::New);
//! assert_eq!(machine.trigger(&state, Event::Start).unwrap(), State::Running);
//! assert_eq!(state.get(), State::Running);
//! ```
//!
//! # Author
//!
//! Haixing Hu

#![deny(missing_docs)]

mod state_cell;
mod state_machine;
mod state_machine_build_error;
mod state_machine_builder;
mod state_machine_error;
mod transition;

pub use state_cell::StateCell;
pub use state_machine::StateMachine;
pub use state_machine_build_error::StateMachineBuildError;
pub use state_machine_builder::StateMachineBuilder;
pub use state_machine_error::StateMachineError;
pub use transition::Transition;
