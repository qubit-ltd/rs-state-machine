/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for state machine construction and rule validation.

use qubit_state_machine::{StateMachine, StateMachineBuildError, Transition};

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

fn create_valid_builder() -> qubit_state_machine::StateMachineBuilder<JobState, JobEvent> {
    let mut builder = StateMachine::builder();
    builder.add_states(&[
        JobState::New,
        JobState::Running,
        JobState::Done,
        JobState::Failed,
    ]);
    builder.set_initial_state(JobState::New);
    builder.set_final_states(&[JobState::Done, JobState::Failed]);
    builder.add_transition(JobState::New, JobEvent::Start, JobState::Running);
    builder.add_transition(JobState::Running, JobEvent::Finish, JobState::Done);
    builder.add_transition(JobState::Running, JobEvent::Fail, JobState::Failed);
    builder
}

#[test]
fn test_builder_build_creates_immutable_state_machine() {
    let machine = create_valid_builder()
        .build()
        .expect("valid state machine should build");

    assert_eq!(machine.states().len(), 4);
    assert_eq!(machine.initial_states().len(), 1);
    assert_eq!(machine.final_states().len(), 2);
    assert_eq!(machine.transitions().len(), 3);
    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::New));
    assert!(machine.is_final_state(JobState::Done));
    assert!(!machine.is_final_state(JobState::Running));
    assert_eq!(
        machine.transition_target(JobState::New, JobEvent::Start),
        Some(JobState::Running)
    );
    assert_eq!(
        machine.transition_target(JobState::New, JobEvent::Finish),
        None
    );
}

#[test]
fn test_builder_add_transition_value_accepts_transition_object() {
    let mut builder = StateMachine::builder();
    builder.add_states(&[JobState::New, JobState::Running]);
    builder.set_initial_state(JobState::New);
    builder.add_transition_value(Transition::new(
        JobState::New,
        JobEvent::Start,
        JobState::Running,
    ));

    let machine = builder.build().expect("transition object should build");

    assert_eq!(
        machine.transition_target(JobState::New, JobEvent::Start),
        Some(JobState::Running)
    );
}

#[test]
fn test_builder_default_matches_new_builder() {
    let mut builder: qubit_state_machine::StateMachineBuilder<JobState, JobEvent> =
        qubit_state_machine::StateMachineBuilder::default();
    builder.add_state(JobState::New);
    builder.set_initial_state(JobState::New);

    let machine = builder.build().expect("single-state machine should build");

    assert!(machine.contains_state(JobState::New));
    assert!(machine.is_initial_state(JobState::New));
}

#[test]
fn test_builder_set_initial_states_registers_multiple_initial_states() {
    let mut builder: qubit_state_machine::StateMachineBuilder<JobState, JobEvent> =
        StateMachine::builder();
    builder.add_states(&[JobState::New, JobState::Running]);
    builder.set_initial_states(&[JobState::New, JobState::Running]);

    let machine = builder
        .build()
        .expect("multiple initial states should build");

    assert!(machine.is_initial_state(JobState::New));
    assert!(machine.is_initial_state(JobState::Running));
}

#[test]
fn test_builder_build_rejects_unregistered_initial_state() {
    let mut builder: qubit_state_machine::StateMachineBuilder<JobState, JobEvent> =
        StateMachine::builder();
    builder.add_state(JobState::Running);
    builder.set_initial_state(JobState::New);

    let error = builder
        .build()
        .expect_err("unregistered initial state should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::InitialStateNotRegistered {
            state: JobState::New
        }
    );
}

#[test]
fn test_build_error_display_describes_each_variant() {
    assert_eq!(
        StateMachineBuildError::<JobState, JobEvent>::InitialStateNotRegistered {
            state: JobState::New
        }
        .to_string(),
        "initial state is not registered: New"
    );
    assert_eq!(
        StateMachineBuildError::<JobState, JobEvent>::FinalStateNotRegistered {
            state: JobState::Done
        }
        .to_string(),
        "final state is not registered: Done"
    );
    assert_eq!(
        StateMachineBuildError::TransitionSourceNotRegistered {
            source: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
        .to_string(),
        "transition source is not registered: New --Start--> Running"
    );
    assert_eq!(
        StateMachineBuildError::TransitionTargetNotRegistered {
            source: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
        .to_string(),
        "transition target is not registered: New --Start--> Running"
    );
    assert_eq!(
        StateMachineBuildError::DuplicateTransition {
            source: JobState::New,
            event: JobEvent::Start,
            existing_target: JobState::Running,
            new_target: JobState::Detached,
        }
        .to_string(),
        "duplicate transition target: New --Start--> Running conflicts with Detached"
    );
}

#[test]
fn test_builder_build_rejects_unregistered_final_state() {
    let mut builder: qubit_state_machine::StateMachineBuilder<JobState, JobEvent> =
        StateMachine::builder();
    builder.add_state(JobState::Running);
    builder.set_final_state(JobState::Done);

    let error = builder
        .build()
        .expect_err("unregistered final state should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::FinalStateNotRegistered {
            state: JobState::Done
        }
    );
}

#[test]
fn test_builder_build_rejects_transition_with_unknown_source() {
    let mut builder = StateMachine::builder();
    builder.add_state(JobState::Running);
    builder.add_transition(JobState::New, JobEvent::Start, JobState::Running);

    let error = builder
        .build()
        .expect_err("unregistered transition source should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::TransitionSourceNotRegistered {
            source: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
    );
}

#[test]
fn test_builder_build_rejects_transition_with_unknown_target() {
    let mut builder = StateMachine::builder();
    builder.add_state(JobState::New);
    builder.add_transition(JobState::New, JobEvent::Start, JobState::Running);

    let error = builder
        .build()
        .expect_err("unregistered transition target should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::TransitionTargetNotRegistered {
            source: JobState::New,
            event: JobEvent::Start,
            target: JobState::Running,
        }
    );
}

#[test]
fn test_builder_build_rejects_conflicting_transition_targets() {
    let mut builder = create_valid_builder();
    builder.add_state(JobState::Detached);
    builder.add_transition(JobState::New, JobEvent::Start, JobState::Detached);

    let error = builder
        .build()
        .expect_err("conflicting transitions should be rejected");

    assert_eq!(
        error,
        StateMachineBuildError::DuplicateTransition {
            source: JobState::New,
            event: JobEvent::Start,
            existing_target: JobState::Running,
            new_target: JobState::Detached,
        }
    );
}
