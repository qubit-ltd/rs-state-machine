//! Builder-level configuration must preserve last-setter precedence.

use qubit_atomic::AtomicRef;
use qubit_cas::CasDecision;
use qubit_cas::CasExecutor;
use qubit_cas::CasStrategy;

use crate::STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS;
use crate::StateMachine;
use crate::StateMachineError;

#[test]
fn test_default_executor_is_attempt_bounded_without_time_budgets() {
    let machine = StateMachine::<u8, u8>::builder()
        .add_states(&[0, 1])
        .initial_state(0)
        .transition(0, 0, 1)
        .build()
        .expect("valid machine");
    let executor = machine.cas_executor();
    let limits = executor;
    assert_eq!(STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS, 16);
    assert_eq!(limits.max_attempts(), STATE_MACHINE_DEFAULT_CAS_MAX_ATTEMPTS);
    assert_eq!(limits.max_operation_elapsed(), None);
    assert_eq!(limits.max_total_elapsed(), None);
    assert_eq!(executor.attempt_timeout(), None);
    assert_eq!(executor.flow_timeout(), None);
}

#[test]
fn test_injection_and_strategy_replace_whole_executor() {
    for strategy_last in [false, true] {
        let executor = CasExecutor::<u8, StateMachineError<u8, u8>>::builder()
            .max_attempts(1)
            .build()
            .expect("valid executor");
        let builder = StateMachine::<u8, u8>::builder();
        let builder = if strategy_last {
            builder.cas_executor(executor).cas_strategy(CasStrategy::LatencyFirst)
        } else {
            builder.cas_strategy(CasStrategy::LatencyFirst).cas_executor(executor)
        };
        let limits = &builder.cas_executor;
        if strategy_last {
            let profile = CasStrategy::LatencyFirst.profile();
            assert_eq!(limits.max_attempts(), profile.max_attempts());
            assert_eq!(limits.max_operation_elapsed(), Some(profile.max_operation_elapsed()));
            assert_eq!(limits.max_total_elapsed(), profile.max_total_elapsed());
        } else {
            assert_eq!(limits.max_attempts(), 1);
            assert_eq!(limits.max_operation_elapsed(), None);
            assert_eq!(limits.max_total_elapsed(), None);
        }
        if !strategy_last {
            let state = AtomicRef::from_value(0u8);
            let error = builder
                .cas_executor
                .execute_result(&state, |_: &u8| {
                    CasDecision::<u8, (), StateMachineError<u8, u8>>::retry(StateMachineError::UnknownState {
                        state: 0,
                    })
                })
                .expect_err("injected single attempt exhausts");
            assert_eq!(error.attempts(), 1);
        }
    }
}
