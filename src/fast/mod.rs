// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! High-performance state machine implementation based on compact `u64` state
//! codes.
//!
//! `FastStateMachine` is designed for hot paths where state and event are
//! represented as dense integer codes and transition lookup must be
//! constant-time. It stores transition rules in a flat table and applies
//! updates with [`qubit_fast_cas::FastCas`].

mod fast_state_machine;
mod fast_state_machine_build_error;
mod fast_state_machine_builder;
mod fast_state_machine_error;

pub use fast_state_machine::FastStateMachine;
pub use fast_state_machine_build_error::FastStateMachineBuildError;
pub use fast_state_machine_builder::FAST_STATE_MACHINE_DEFAULT_CAS_POLICY;
pub use fast_state_machine_builder::FastStateMachineBuilder;
pub use fast_state_machine_error::FastStateMachineError;
pub use fast_state_machine_error::FastStateMachineResult;

mod typed;
pub use typed::DenseCode;
pub use typed::TypedFastState;
pub use typed::TypedFastStateMachine;
pub use typed::TypedFastStateMachineBuildError;
pub use typed::TypedFastStateMachineBuilder;
pub use typed::TypedFastStateMachineError;
