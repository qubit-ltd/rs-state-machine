/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Integration tests for `qubit-state-machine`.

#[path = "state_machine_builder/state_machine_builder_tests.rs"]
mod state_machine_builder_tests;

mod state_machine_build_error_tests;
mod state_machine_error_tests;

#[path = "state_machine/state_machine_tests.rs"]
mod state_machine_tests;

#[path = "transition/transition_tests.rs"]
mod transition_tests;
