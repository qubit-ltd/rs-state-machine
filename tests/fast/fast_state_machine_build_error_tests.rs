/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for fast state machine build-time errors.

use qubit_state_machine::FastStateMachineBuildError;

#[test]
fn test_build_error_display_reports_all_variants() {
    assert_eq!(
        FastStateMachineBuildError::StateCountNotConfigured.to_string(),
        "state count is not configured",
    );
    assert_eq!(
        FastStateMachineBuildError::EventCountNotConfigured.to_string(),
        "event count is not configured",
    );
    assert_eq!(
        FastStateMachineBuildError::InvalidStateCount { count: 0 }.to_string(),
        "state count must be positive: 0",
    );
    assert_eq!(
        FastStateMachineBuildError::InvalidEventCount { count: 0 }.to_string(),
        "event count must be positive: 0",
    );
    assert_eq!(
        FastStateMachineBuildError::TransitionTableOverflow {
            state_count: usize::MAX,
            event_count: 2
        }
        .to_string(),
        "transition table overflowed usize: 18446744073709551615 * 2",
    );
    assert_eq!(
        FastStateMachineBuildError::InitialStateOutOfRange {
            state: 2,
            state_count: 2,
        }
        .to_string(),
        "initial state is out of range: 2 >= 2",
    );
    assert_eq!(
        FastStateMachineBuildError::FinalStateOutOfRange {
            state: 2,
            state_count: 2,
        }
        .to_string(),
        "final state is out of range: 2 >= 2",
    );
    assert_eq!(
        FastStateMachineBuildError::TransitionSourceOutOfRange {
            source_state: 2,
            state_count: 2,
        }
        .to_string(),
        "transition source is out of range: 2 >= 2",
    );
    assert_eq!(
        FastStateMachineBuildError::TransitionEventOutOfRange {
            event: 2,
            event_count: 2,
        }
        .to_string(),
        "transition event is out of range: 2 >= 2",
    );
    assert_eq!(
        FastStateMachineBuildError::TransitionTargetOutOfRange {
            target: 2,
            state_count: 2,
        }
        .to_string(),
        "transition target is out of range: 2 >= 2",
    );
    assert_eq!(
        FastStateMachineBuildError::DuplicateTransition {
            source_state: 0,
            event: 1,
            existing_target: 0,
            new_target: 1,
        }
        .to_string(),
        "duplicate transition: 0 --1--> 0 conflicts with 1",
    );
}

#[test]
fn test_build_error_equality_uses_variant_fields() {
    let first = FastStateMachineBuildError::DuplicateTransition {
        source_state: 0,
        event: 1,
        existing_target: 0,
        new_target: 1,
    };
    let second = FastStateMachineBuildError::DuplicateTransition {
        source_state: 0,
        event: 1,
        existing_target: 0,
        new_target: 1,
    };
    let different = FastStateMachineBuildError::DuplicateTransition {
        source_state: 0,
        event: 1,
        existing_target: 1,
        new_target: 0,
    };

    assert_eq!(first, second);
    assert_ne!(first, different);
}
