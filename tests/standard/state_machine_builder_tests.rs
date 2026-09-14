// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for state machine construction and rule validation.

use qubit_cas::CasStrategy;
use qubit_state_machine::StateMachine;
use qubit_state_machine::StateMachineBuildError;
use qubit_state_machine::StateMachineBuilder;
use qubit_state_machine::Transition;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    New,
    Running,
    Done,
    Failed,
    Detached,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Finish,
    Fail,
}

fn create_valid_builder() -> StateMachineBuilder<JobState, JobEvent> {
    StateMachine::builder()
        .add_states(&[JobState::New, JobState::Running, JobState::Done, JobState::Failed])
        .initial_state(JobState::New)
        .terminal_states(&[JobState::Done, JobState::Failed])
        .transition(JobState::New, JobEvent::Start, JobState::Running)
        .transition(JobState::Running, JobEvent::Finish, JobState::Done)
        .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
}

#[test]
fn test_builder_build_creates_immutable_state_machine() {
    let machine = create_valid_builder()
        .build()
        .expect("valid state machine should build");

    assert_eq!(machine.states().len(), 4);
    assert_eq!(machine.state_count(), 4);
    assert_eq!(machine.initial_state(), JobState::New);
    assert_eq!(machine.terminal_states().len(), 2);
    assert_eq!(machine.transitions().collect::<Vec<_>>().len(), 3);
    assert_eq!(machine.transition_count(), 3);
    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::New));
    assert!(machine.is_terminal_state(JobState::Done));
    assert!(!machine.is_terminal_state(JobState::Running));
    assert_eq!(
        machine.transition_target(JobState::New, JobEvent::Start),
        Some(JobState::Running)
    );
    assert_eq!(machine.transition_target(JobState::New, JobEvent::Finish), None);
    assert_eq!(*machine.create_state().load(), JobState::New);
}

#[test]
fn test_builder_accepts_cas_strategy_configuration() {
    let _machine = create_valid_builder()
        .cas_strategy(CasStrategy::ReliabilityFirst)
        .build()
        .expect("strategy-configured machine should build");
}

#[test]
fn test_builder_build_supports_chained_rule_definition() {
    let machine = StateMachine::builder()
        .add_states(&[JobState::New, JobState::Running, JobState::Done, JobState::Failed])
        .initial_state(JobState::New)
        .terminal_states(&[JobState::Done, JobState::Failed])
        .transition(JobState::New, JobEvent::Start, JobState::Running)
        .transition(JobState::Running, JobEvent::Finish, JobState::Done)
        .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .build()
        .expect("chained builder should build valid rules");

    assert_eq!(machine.states().len(), 4);
    assert!(machine.is_initial_state(JobState::New));
    assert!(machine.is_terminal_state(JobState::Done));
    assert_eq!(
        machine.transition_target(JobState::Running, JobEvent::Finish),
        Some(JobState::Done)
    );
}

#[test]
fn test_builder_build_accepts_exact_duplicate_transition() {
    let builder = create_valid_builder().transition(JobState::New, JobEvent::Start, JobState::Running);

    let machine = builder.build().expect("exact duplicate transition should build");

    assert!(machine.contains_state(JobState::New));
    assert!(machine.is_initial_state(JobState::New));
    assert!(machine.is_terminal_state(JobState::Done));
    assert_eq!(machine.transitions().count(), 3);
    assert_eq!(
        machine.transition_target(JobState::Running, JobEvent::Finish),
        Some(JobState::Done)
    );
}

#[test]
fn test_builder_transition_value_accepts_transition_object() {
    let builder = StateMachine::builder()
        .add_states(&[JobState::New, JobState::Running])
        .initial_state(JobState::New)
        .transition_value(Transition::new(JobState::New, JobEvent::Start, JobState::Running));

    let machine = builder.build().expect("transition object should build");

    assert_eq!(
        machine.transition_target(JobState::New, JobEvent::Start),
        Some(JobState::Running)
    );
}

#[test]
fn test_builder_default_matches_new_builder() {
    let builder: StateMachineBuilder<JobState, JobEvent> = StateMachineBuilder::default()
        .add_state(JobState::New)
        .initial_state(JobState::New);

    let machine = builder.build().expect("single-state machine should build");

    assert!(machine.contains_state(JobState::New));
    assert!(machine.is_initial_state(JobState::New));
}

#[test]
fn test_builder_initial_state_is_unique() {
    let builder: StateMachineBuilder<JobState, JobEvent> = StateMachine::builder()
        .add_states(&[JobState::New, JobState::Running])
        .initial_state(JobState::Running);

    let machine = builder.build().expect("multiple initial states should build");

    assert!(machine.is_initial_state(JobState::Running));
}

#[test]
fn test_builder_build_rejects_unregistered_initial_state() {
    let builder: StateMachineBuilder<JobState, JobEvent> = StateMachine::builder()
        .add_state(JobState::Running)
        .initial_state(JobState::New);

    let error = builder
        .build()
        .expect_err("unregistered initial state should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::InitialStateNotRegistered { state: JobState::New }
    );
}

#[test]
fn test_build_error_display_describes_each_variant() {
    assert_eq!(
        StateMachineBuildError::<JobState, JobEvent>::InitialStateNotRegistered { state: JobState::New }.to_string(),
        "initial state is not registered: New"
    );
    assert_eq!(
        StateMachineBuildError::<JobState, JobEvent>::TerminalStateNotRegistered { state: JobState::Done }.to_string(),
        "terminal state is not registered: Done"
    );
    assert_eq!(
        StateMachineBuildError::TransitionSourceNotRegistered {
            source_state: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
        .to_string(),
        "transition source is not registered: New --Start--> Running"
    );
    assert_eq!(
        StateMachineBuildError::TransitionTargetNotRegistered {
            source_state: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
        .to_string(),
        "transition target is not registered: New --Start--> Running"
    );
    assert_eq!(
        StateMachineBuildError::DuplicateTransition {
            source_state: JobState::New,
            event: JobEvent::Start,
            existing_target: JobState::Running,
            new_target: JobState::Detached,
        }
        .to_string(),
        "duplicate transition target: New --Start--> Running conflicts with Detached"
    );
}

#[test]
fn test_builder_build_rejects_unregistered_terminal_state() {
    let builder: StateMachineBuilder<JobState, JobEvent> = StateMachine::builder()
        .add_state(JobState::Running)
        .initial_state(JobState::Running)
        .terminal_state(JobState::Done);

    let error = builder
        .build()
        .expect_err("unregistered final state should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::TerminalStateNotRegistered { state: JobState::Done }
    );
}

#[test]
fn test_builder_reports_first_unregistered_terminal_in_configuration_order() {
    for _ in 0..64 {
        let error = StateMachine::<JobState, JobEvent>::builder()
            .add_state(JobState::Running)
            .initial_state(JobState::Running)
            .terminal_states(&[JobState::Done, JobState::Failed, JobState::Done])
            .build()
            .expect_err("both terminal states are unregistered");

        assert_eq!(
            error,
            StateMachineBuildError::TerminalStateNotRegistered { state: JobState::Done }
        );
    }
}

#[test]
fn test_builder_build_rejects_transition_with_unknown_source() {
    let builder = StateMachine::builder()
        .add_state(JobState::Running)
        .initial_state(JobState::Running)
        .transition(JobState::New, JobEvent::Start, JobState::Running);

    let error = builder
        .build()
        .expect_err("unregistered transition source should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::TransitionSourceNotRegistered {
            source_state: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
    );
}

#[test]
fn test_builder_build_rejects_transition_with_unknown_target() {
    let builder = StateMachine::builder()
        .add_state(JobState::New)
        .initial_state(JobState::New)
        .transition(JobState::New, JobEvent::Start, JobState::Running);

    let error = builder
        .build()
        .expect_err("unregistered transition target should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::TransitionTargetNotRegistered {
            source_state: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
    );
}

#[test]
fn test_builder_build_rejects_conflicting_transition_targets() {
    let builder = create_valid_builder().add_state(JobState::Detached).transition(
        JobState::New,
        JobEvent::Start,
        JobState::Detached,
    );

    let error = builder.build().expect_err("conflicting transitions should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::DuplicateTransition {
            source_state: JobState::New,
            event: JobEvent::Start,
            existing_target: JobState::Running,
            new_target: JobState::Detached,
        }
    );
}
