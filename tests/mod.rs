/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Integration tests for `qubit-state-machine`.

#[path = "state_machine_builder/state_machine_builder_tests.rs"]
mod state_machine_builder_tests;

#[path = "state_machine/state_machine_tests.rs"]
mod state_machine_tests;

#[path = "transition/transition_tests.rs"]
mod transition_tests;
