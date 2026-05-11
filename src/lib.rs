/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *    You may obtain a copy of the License at
 *
 *        http://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 *
 ******************************************************************************/
//! # Qubit State Machine
//!
//! A small, thread-safe finite state machine for Rust.
//!
//! This crate provides a generic state machine (`StateMachine`) and a compact
//! fast state machine (`FastStateMachine`) built on compact `usize` codes and
//! `FastCas`.
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
//! let machine = StateMachine::builder()
//!     .add_states(&[State::New, State::Running, State::Done])
//!     .set_initial_state(State::New)
//!     .set_final_state(State::Done)
//!     .add_transition(State::New, Event::Start, State::Running)
//!     .add_transition(State::Running, Event::Finish, State::Done)
//!     .build()
//!     .expect("job state machine should be valid");
//!
//! let state = AtomicRef::from_value(State::New);
//! assert_eq!(machine.trigger(&state, Event::Start).unwrap(), State::Running);
//! assert_eq!(*state.load(), State::Running);
//! ```

#![deny(missing_docs)]

mod fast;
mod standard;

pub use qubit_atomic::AtomicRef;
pub use qubit_cas::FastCasPolicy;

pub use fast::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastStateMachine,
    FastStateMachineBuildError,
    FastStateMachineBuilder,
    FastStateMachineError,
    FastStateMachineResult,
};

pub use standard::{
    StateMachine,
    StateMachineBuildError,
    StateMachineBuilder,
    StateMachineError,
    StateMachineResult,
    Transition,
};
