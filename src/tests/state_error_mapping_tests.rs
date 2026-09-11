// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Projection tests requiring the private state-machine error adapter.

use std::sync::Arc;
use std::time::Duration;

use qubit_atomic::AtomicRef;
use qubit_cas::CasDecision;
use qubit_cas::CasErrorKind;
use qubit_cas::CasExecutor;

use super::StateMachine;
use crate::StateMachineError;

#[test]
fn test_budget_failures_preserve_kind_and_attempt_count() {
    for operation_budget in [false, true] {
        let builder = CasExecutor::<u8, StateMachineError<u8, u8>>::builder();
        let executor = if operation_budget {
            builder.max_operation_elapsed(Some(Duration::from_millis(1)))
        } else {
            builder.max_total_elapsed(Some(Duration::from_millis(1)))
        }
        .build()
        .expect("valid budget");
        let state = AtomicRef::from_value(0u8);
        let error = executor
            .execute_result(&state, |_: &u8| {
                std::thread::sleep(Duration::from_millis(5));
                state.store(Arc::new(1));
                CasDecision::update(2, ())
            })
            .expect_err("budget prevents retry after conflict");
        assert_eq!(
            StateMachine::<u8, u8>::state_error_from_cas_error(error),
            StateMachineError::CasFailure {
                kind: if operation_budget {
                    CasErrorKind::OperationBudgetExceeded
                } else {
                    CasErrorKind::TotalBudgetExceeded
                },
                attempts: 1,
            }
        );
    }
}

#[test]
fn test_business_abort_unwraps_original_error() {
    let business = StateMachineError::UnknownState { state: 7u8 };
    let state = AtomicRef::from_value(0u8);
    let executor = CasExecutor::<u8, StateMachineError<u8, u8>>::builder()
        .build()
        .expect("valid policy");
    let error = executor
        .execute_result(&state, |_: &u8| CasDecision::<u8, (), _>::abort(business))
        .expect_err("abort");
    assert_eq!(StateMachine::<u8, u8>::state_error_from_cas_error(error), business);
}
