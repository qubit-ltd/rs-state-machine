// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! # Qubit State Machine
//!
//! A small, thread-safe finite state machine for Rust.
//!
//! With the default feature set, this crate provides a generic state machine
//! (`StateMachine`) and a compact fast state machine (`FastStateMachine`)
//! built on dense `u64` codes and `FastCas`.
//!
//! # Cargo features
//!
//! - `standard` enables the generic `StateMachine` implementation and its
//!   `qubit-atomic`/`qubit-cas` dependencies.
//! - `fast` enables the `FastStateMachine` implementation and only its
//!   `qubit-fast-cas` dependency.
//! - The default feature set enables both implementations.
//!
//! Atomic state types remain owned by their respective crates. Import
//! `qubit_atomic::AtomicRef` or `qubit_fast_cas::FastCasState` directly
//! instead of through this crate.
//!
//! # Examples
//!
//! ```
//! # #[cfg(feature = "standard")]
//! # {
//! use qubit_atomic::AtomicRef;
//! use qubit_state_machine::StateMachine;
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
//! let machine = StateMachine::builder()
//!     .add_states(&[State::New, State::Running, State::Done])
//!     .initial_state(State::New)
//!     .terminal_state(State::Done)
//!     .transition(State::New, Event::Start, State::Running)
//!     .transition(State::Running, Event::Finish, State::Done)
//!     .build()
//!     .expect("job state machine should be valid");
//!
//! let state = AtomicRef::from_value(State::New);
//! assert_eq!(machine.trigger(&state, Event::Start).unwrap(), State::Running);
//! assert_eq!(*state.load(), State::Running);
//! # }
//! ```

#![deny(missing_docs)]

mod transition;

pub use transition::Transition;

#[cfg(feature = "fast")]
mod fast;
#[cfg(feature = "standard")]
mod standard;

#[cfg(feature = "fast")]
pub use fast::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY;
#[cfg(feature = "fast")]
pub use fast::FastStateMachine;
#[cfg(feature = "fast")]
pub use fast::FastStateMachineBuildError;
#[cfg(feature = "fast")]
pub use fast::FastStateMachineBuilder;
#[cfg(feature = "fast")]
pub use fast::FastStateMachineError;
#[cfg(feature = "fast")]
pub use fast::FastStateMachineResult;
#[cfg(feature = "standard")]
pub use standard::StateMachine;
#[cfg(feature = "standard")]
pub use standard::StateMachineBuildError;
#[cfg(feature = "standard")]
pub use standard::StateMachineBuilder;
#[cfg(feature = "standard")]
pub use standard::StateMachineError;
#[cfg(feature = "standard")]
pub use standard::StateMachineResult;
