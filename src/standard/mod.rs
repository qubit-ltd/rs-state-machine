/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/

//! Standard, fully generic state machine implementation.
//!
//! This module keeps the public generic API based on user-provided state and
//! event enums. It is optimized for correctness and ergonomic API while
//! preserving the current crate behavior.

mod state_machine;
mod state_machine_build_error;
mod state_machine_builder;
mod state_machine_error;
mod transition;

pub use state_machine::StateMachine;
pub use state_machine_build_error::StateMachineBuildError;
pub use state_machine_builder::StateMachineBuilder;
pub use state_machine_error::{
    StateMachineError,
    StateMachineResult,
};
pub use transition::Transition;
