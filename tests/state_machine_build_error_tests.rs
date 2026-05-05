/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for state machine build errors.

use std::error::Error;

use qubit_state_machine::StateMachineBuildError;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestState {
    New,
    Running,
    Done,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum TestEvent {
    Start,
    Finish,
}

#[test]
fn test_state_machine_build_error_equality_uses_variant_fields() {
    let error = StateMachineBuildError::DuplicateTransition {
        source: TestState::New,
        event: TestEvent::Start,
        existing_target: TestState::Running,
        new_target: TestState::Done,
    };
    let same = StateMachineBuildError::DuplicateTransition {
        source: TestState::New,
        event: TestEvent::Start,
        existing_target: TestState::Running,
        new_target: TestState::Done,
    };
    let different = StateMachineBuildError::DuplicateTransition {
        source: TestState::New,
        event: TestEvent::Start,
        existing_target: TestState::Done,
        new_target: TestState::Running,
    };

    assert_eq!(error, same);
    assert_ne!(error, different);
}

#[test]
fn test_state_machine_build_error_display_reports_validation_context() {
    assert_eq!(
        StateMachineBuildError::<TestState, TestEvent>::InitialStateNotRegistered {
            state: TestState::New,
        }
        .to_string(),
        "initial state is not registered: New"
    );
    assert_eq!(
        StateMachineBuildError::TransitionTargetNotRegistered {
            source: TestState::New,
            event: TestEvent::Finish,
            target: TestState::Done,
        }
        .to_string(),
        "transition target is not registered: New --Finish--> Done"
    );
}

#[test]
fn test_state_machine_build_error_has_no_nested_source() {
    let error = StateMachineBuildError::<TestState, TestEvent>::FinalStateNotRegistered {
        state: TestState::Done,
    };

    assert!(error.source().is_none());
}
