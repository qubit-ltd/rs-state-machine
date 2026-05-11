# Qubit State Machine (`rs-state-machine`)

[![Rust CI](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml)
[![Coverage Status](https://coveralls.io/repos/github/qubit-ltd/rs-state-machine/badge.svg?branch=main)](https://coveralls.io/github/qubit-ltd/rs-state-machine?branch=main)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Docs.rs](https://docs.rs/qubit-state-machine/badge.svg)](https://docs.rs/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Documentation: [API Reference](https://docs.rs/qubit-state-machine)

`qubit-state-machine` is a small Rust finite state machine crate for lifecycle,
workflow, and task-state tracking code.

It provides immutable transition rules, validation at build time, and a
CAS-backed `AtomicRef` for applying events to shared state.

For performance-critical hot paths, it also provides
[`FastStateMachine`], a compact integer-based implementation that stores transitions
in a fixed table and updates state through `FastCas`.

## Why Use It

Use `qubit-state-machine` when you need:

- explicit finite state machine rules built from enum-like state and event types
- immutable transition tables that can be shared across threads
- build-time validation for unknown states and conflicting transitions
- event-driven state updates through `trigger` and `try_trigger`
- success callbacks that observe the old and new state after an update
- simple state tracking for services, jobs, devices, or UI logic
- dense integer state/event transitions through [`FastStateMachine`] when you need
  predictable low-latency path performance

## Installation

```toml
[dependencies]
qubit-state-machine = "0.2"
```

## Quick Start: Job Processing

```rust
use qubit_state_machine::{AtomicRef, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Complete,
    Fail,
}

fn create_job_machine() -> Result<StateMachine<JobState, JobEvent>, Box<dyn std::error::Error>> {
    Ok(StateMachine::builder()
        .add_states(&[
            JobState::Queued,
            JobState::Running,
            JobState::Succeeded,
            JobState::Failed,
        ])
        .set_initial_state(JobState::Queued)
        .set_final_states(&[JobState::Succeeded, JobState::Failed])
        .add_transition(JobState::Queued, JobEvent::Start, JobState::Running)
        .add_transition(JobState::Running, JobEvent::Complete, JobState::Succeeded)
        .add_transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .build()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let machine = create_job_machine()?;

    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::Queued));
    assert!(machine.is_final_state(JobState::Succeeded));
    assert_eq!(
        machine.transition_target(JobState::Queued, JobEvent::Start),
        Some(JobState::Running),
    );

    let state = AtomicRef::from_value(JobState::Queued);
    let running = machine.trigger(&state, JobEvent::Start)?;
    assert_eq!(running, JobState::Running);
    assert_eq!(*state.load(), JobState::Running);

    let mut audit_log = Vec::new();
    let finished = machine.trigger_with(&state, JobEvent::Complete, |old_state, new_state| {
        audit_log.push((old_state, new_state));
    })?;
    assert_eq!(finished, JobState::Succeeded);
    assert_eq!(audit_log, vec![(JobState::Running, JobState::Succeeded)]);

    assert!(!machine.try_trigger(&state, JobEvent::Fail));
    assert_eq!(*state.load(), JobState::Succeeded);

    Ok(())
}
```

## Fast State Machine

`FastStateMachine` is intended for dense integer codes and high-throughput
dispatch loops. It validates the complete transition table at build time and keeps
runtime transition logic in O(1) by indexing a flat transition array.

```rust
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastCasPolicy,
    FastStateMachine,
};

const QUEUED: usize = 0;
const RUNNING: usize = 1;
const SUCCEEDED: usize = 2;
const FAILED: usize = 3;
const START: usize = 0;
const COMPLETE: usize = 1;
const FAIL: usize = 2;

let machine = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .final_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .cas_policy(FastCasPolicy::spin(8))
    .build()?;

let state = qubit_cas::FastCasState::new(QUEUED);
assert_eq!(machine.trigger(&state, START)?, RUNNING);
assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
assert_eq!(machine.state_count(), 4);
assert_eq!(machine.event_count(), 3);
assert!(machine.is_initial_state(QUEUED));
assert!(machine.is_final_state(SUCCEEDED));
assert_eq!(machine.cas_policy(), FAST_STATE_MACHINE_DEFAULT_CAS_POLICY);
```

Default policy is `FAST_STATE_MACHINE_DEFAULT_CAS_POLICY` so callers can start
from safe defaults and only override with `.cas_policy(...)` when needed.

## Build-Time Validation

Invalid rules are rejected before a `StateMachine` is created.

```rust
use qubit_state_machine::{StateMachine, StateMachineBuildError};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    Queued,
    Running,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
}

let error = StateMachine::builder()
    .add_state(JobState::Queued)
    .add_transition(JobState::Queued, JobEvent::Start, JobState::Running)
    .build()
    .expect_err("transition target must be registered");

assert_eq!(
    error,
    StateMachineBuildError::TransitionTargetNotRegistered {
        source_state: JobState::Queued,
        event: JobEvent::Start,
        target: JobState::Running,
    },
);
```

## Applying Events Without Error Handling

Use `try_trigger` or `try_trigger_with` when an invalid transition should be a
simple `false` result.

```rust
use qubit_state_machine::{AtomicRef, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum DoorState {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum DoorEvent {
    Close,
    Reopen,
}

let machine = StateMachine::builder()
    .add_states(&[DoorState::Open, DoorState::Closed])
    .add_transition(DoorState::Open, DoorEvent::Close, DoorState::Closed)
    .build()
    .expect("rules should build");
let state = AtomicRef::from_value(DoorState::Open);

assert!(machine.try_trigger(&state, DoorEvent::Close));
assert!(!machine.try_trigger_with(&state, DoorEvent::Reopen, |_, _| {
    unreachable!("callback is skipped when transition fails");
}));

assert_eq!(*state.load(), DoorState::Closed);
```

## Common Next Steps

| Task | API |
| --- | --- |
| Define states and transitions | `StateMachine::builder`, `StateMachineBuilder` |
| Define dense fast machines | `FastStateMachine::builder`, `FastStateMachineBuilder` |
| Add one or more states | `add_state`, `add_states` |
| Mark initial and final states | `set_initial_state`, `set_initial_states`, `set_final_state`, `set_final_states` |
| Add transition rules | `add_transition`, `add_transition_value`, `Transition` |
| Query transition targets without changing state | `transition_target` |
| Apply events and get detailed errors | `trigger`, `trigger_with`, `StateMachineError` |
| Apply events without handling errors | `try_trigger`, `try_trigger_with` |
| Store shared mutable state | `AtomicRef` |

## Core API At A Glance

| Type | Purpose |
| --- | --- |
| `Transition` | Immutable value describing `source --event--> target`. |
| `FastStateMachine` | Dense integer-coded transition machine for high-throughput scenarios. |
| `FastStateMachineBuilder` | Builder for state/event code counts, transition table, and CAS policy. |
| `FastStateMachineError` | Runtime error from fast transition execution. |
| `FastStateMachineBuildError` | Build-time validation error for fast transition table configuration. |
| `FastCasPolicy` | Optional CAS retry policy to control contention behavior. |
| `StateMachineBuilder` | Mutable builder for states, initial states, final states, and transitions. |
| `StateMachine` | Immutable, validated transition table used to query and trigger events. |
| `AtomicRef` | Re-exported atomic reference used for CAS-backed current state. |
| `StateMachineBuildError` | Validation error returned while building invalid rule sets. |
| `StateMachineError` | Runtime error returned when an event cannot be applied. |

## Project Scope

- `qubit-state-machine` is intended for simple finite state machines, not a full
  workflow engine.
- State and event types should be small enum-like values implementing
  `Copy + Eq + Hash + Debug`.
- Rule definitions become immutable after `StateMachineBuilder::build`.
- `trigger` accepts `AtomicRef<S>` directly.
- Event-driven transitions are installed through `qubit-cas`.
- Callbacks run after the CAS update has succeeded.
- `FastStateMachine` uses compact state/event integer codes and a flat transition
  table for predictable performance.

## Contributing

Issues and pull requests are welcome.

Please keep contributions focused and easy to review:

- open an issue for bug reports, design questions, or larger feature proposals
- keep pull requests scoped to one behavior change, fix, or documentation update
- run `./ci-check.sh` before submitting changes
- include tests when changing runtime behavior
- update the README when public API behavior changes

By contributing to this project, you agree that your contribution will be
licensed under the same license as the project.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
