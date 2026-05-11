/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for fast state machine runtime errors.

use std::error::Error;

use qubit_state_machine::FastStateMachineError;

#[test]
fn test_fast_state_machine_error_display() {
    assert_eq!(
        FastStateMachineError::UnknownState { state: 3 }.to_string(),
        "unknown state: 3",
    );
    assert_eq!(
        FastStateMachineError::UnknownTransition {
            source_state: 1,
            event: 2
        }
        .to_string(),
        "unknown transition: 1 --2--> ?",
    );
    assert_eq!(
        FastStateMachineError::CasConflict { attempts: 7 }.to_string(),
        "CAS transition failed after 7 attempt(s)",
    );
}

#[test]
fn test_fast_state_machine_error_equality_uses_fields() {
    let first = FastStateMachineError::CasConflict { attempts: 1 };
    let second = FastStateMachineError::CasConflict { attempts: 1 };
    let different = FastStateMachineError::CasConflict { attempts: 2 };

    assert_eq!(first, second);
    assert_ne!(first, different);
}

#[test]
fn test_fast_state_machine_error_has_no_nested_source() {
    let error = FastStateMachineError::UnknownTransition {
        source_state: 1,
        event: 1,
    };

    assert!(error.source().is_none());
}
