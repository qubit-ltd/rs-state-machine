/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for runtime state machine errors.

use std::error::Error;

use qubit_state_machine::{
    StateMachineError,
    StateMachineResult,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestState {
    New,
    Running,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestEvent {
    Start,
    Finish,
}

fn create_unknown_state_result() -> StateMachineResult<TestState, TestEvent> {
    Err(StateMachineError::UnknownState {
        state: TestState::New,
    })
}

#[test]
fn test_state_machine_error_equality_uses_variant_fields() {
    let error = StateMachineError::UnknownTransition {
        source: TestState::New,
        event: TestEvent::Start,
    };
    let same = StateMachineError::UnknownTransition {
        source: TestState::New,
        event: TestEvent::Start,
    };
    let different = StateMachineError::UnknownTransition {
        source: TestState::Running,
        event: TestEvent::Finish,
    };

    assert_eq!(error, same);
    assert_ne!(error, different);
}

#[test]
fn test_state_machine_error_display_reports_runtime_context() {
    assert_eq!(
        StateMachineError::<TestState, TestEvent>::UnknownState {
            state: TestState::Running,
        }
        .to_string(),
        "unknown state: Running"
    );
    assert_eq!(
        StateMachineError::UnknownTransition {
            source: TestState::New,
            event: TestEvent::Finish,
        }
        .to_string(),
        "unknown transition: New --Finish--> ?"
    );
    assert_eq!(
        StateMachineError::<TestState, TestEvent>::CasConflict { attempts: 5 }.to_string(),
        "CAS transition failed after 5 attempt(s)"
    );
}

#[test]
fn test_state_machine_result_alias_uses_runtime_error() {
    let result = create_unknown_state_result();

    assert_eq!(
        result.expect_err("result should carry the runtime state machine error"),
        StateMachineError::UnknownState {
            state: TestState::New,
        }
    );
}

#[test]
fn test_state_machine_error_has_no_nested_source() {
    let error = StateMachineError::<TestState, TestEvent>::CasConflict { attempts: 2 };

    assert!(error.source().is_none());
}
