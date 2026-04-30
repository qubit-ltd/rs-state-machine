# Qubit State Machine (`rs-state-machine`)

[![CircleCI](https://circleci.com/gh/qubit-ltd/rs-state-machine.svg?style=shield)](https://circleci.com/gh/qubit-ltd/rs-state-machine)
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
thread-safe `StateCell` for applying events to shared state.

## Why Use It

Use `qubit-state-machine` when you need:

- explicit finite state machine rules built from enum-like state and event types
- immutable transition tables that can be shared across threads
- build-time validation for unknown states and conflicting transitions
- event-driven state updates through `trigger` and `try_trigger`
- success callbacks that observe the old and new state after an update
- simple, dependency-free state tracking for services, jobs, devices, or UI logic

## Installation

```toml
[dependencies]
qubit-state-machine = "0.1.0"
```

## Quick Start

```rust
use qubit_state_machine::{StateCell, StateMachine};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobState {
    New,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum JobEvent {
    Start,
    Finish,
    Fail,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let machine = builder.build()?;
    let state = StateCell::new(JobState::New);

    let running = machine.trigger(&state, JobEvent::Start)?;
    assert_eq!(running, JobState::Running);
    assert_eq!(state.get(), JobState::Running);

    let finished = machine.trigger_with(&state, JobEvent::Finish, |old_state, new_state| {
        println!("state changed: {old_state:?} -> {new_state:?}");
    })?;
    assert_eq!(finished, JobState::Done);

    Ok(())
}
```

## Common Next Steps

| Task | API |
| --- | --- |
| Define states and transitions | `StateMachine::builder`, `StateMachineBuilder` |
| Add one or more states | `add_state`, `add_states` |
| Mark initial and final states | `set_initial_state`, `set_initial_states`, `set_final_state`, `set_final_states` |
| Add transition rules | `add_transition`, `add_transition_value`, `Transition` |
| Query transition targets without changing state | `transition_target` |
| Apply events and get detailed errors | `trigger`, `trigger_with`, `StateMachineError` |
| Apply events without handling errors | `try_trigger`, `try_trigger_with` |
| Store shared mutable state | `StateCell` |

## Core API At A Glance

| Type | Purpose |
| --- | --- |
| `Transition` | Immutable value describing `source --event--> target`. |
| `StateMachineBuilder` | Mutable builder for states, initial states, final states, and transitions. |
| `StateMachine` | Immutable, validated transition table used to query and trigger events. |
| `StateCell` | Thread-safe storage for the current state. |
| `StateMachineBuildError` | Validation error returned while building invalid rule sets. |
| `StateMachineError` | Runtime error returned when an event cannot be applied. |

## Project Scope

- `qubit-state-machine` is intended for simple finite state machines, not a full
  workflow engine.
- State and event types should be small enum-like values implementing
  `Copy + Eq + Hash + Debug`.
- Rule definitions become immutable after `StateMachineBuilder::build`.
- `StateCell` uses a mutex so generic state values can be updated safely across
  threads without requiring platform-specific atomic representations.
- Callbacks run after the state has been updated and after the state lock has
  been released.

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
