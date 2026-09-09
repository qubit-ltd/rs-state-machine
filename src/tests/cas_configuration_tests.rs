//! Builder-level configuration must preserve last-setter precedence.

use qubit_atomic::AtomicRef;
use qubit_cas::CasDecision;
use qubit_cas::CasExecutor;
use qubit_cas::CasStrategy;

use crate::StateMachine;
use crate::StateMachineError;

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
        // Finish has no timing sensitivity; its context exposes the installed limit.
        let state = AtomicRef::from_value(0u8);
        let success = builder
            .cas_executor
            .execute_result(&state, |_: &u8| CasDecision::finish(()))
            .expect("finish succeeds");
        assert_eq!(success.context().max_attempts(), if strategy_last { 100 } else { 1 });
        if !strategy_last {
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
