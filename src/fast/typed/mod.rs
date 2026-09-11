// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//
//! Strongly typed finite encodings over the Fast engine.
mod dense_code;
mod typed_fast_state;
mod typed_fast_state_machine;
mod typed_fast_state_machine_build_error;
mod typed_fast_state_machine_builder;
mod typed_fast_state_machine_error;
pub use dense_code::DenseCode;
pub use typed_fast_state::TypedFastState;
pub use typed_fast_state_machine::TypedFastStateMachine;
pub use typed_fast_state_machine_build_error::TypedFastStateMachineBuildError;
pub use typed_fast_state_machine_builder::TypedFastStateMachineBuilder;
pub use typed_fast_state_machine_error::TypedFastStateMachineError;
