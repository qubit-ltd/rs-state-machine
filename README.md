# Qubit State Machine (`rs-state-machine`)

[![Rust CI](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-state-machine/coverage-badge.json)](https://qubit-ltd.github.io/rs-state-machine/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

The standard state machine uses `qubit-cas` 0.9. Inject limits, budgets, and
backoff with `cas_executor`, or choose a preset with the builder's
`cas_strategy`. Inspect the actual installed configuration through
`machine.cas_executor()` using `max_attempts()`, `max_operation_elapsed()`,
and `max_total_elapsed()`. Terminal CAS kinds are preserved in
`StateMachineError::CasFailure`.

`qubit-state-machine` is a small Rust finite state machine crate for lifecycle,
workflow, and task-state tracking code.

Version 0.9 requires a valid initial state; if `initial_state(...)` is called
more than once, the last value wins. Terminal states cannot have outgoing
transitions. `create_state()` creates an independent current-state
cell; externally created cells are not bound to a machine. Callbacks run once
after a successful commit, with concurrent callback order unspecified, and a
callback may observe a later committed state.

It provides immutable transition rules and build-time validation. The standard
machine updates `qubit_atomic::AtomicRef` values through `qubit-cas`; the Fast
machine updates `qubit_fast_cas::FastCasState` values directly.

There are three public entry points:

- `StateMachine` for clear, generic APIs suitable for enum-like state/event types.
- `FastStateMachine` for high-throughput, integer-coded state/event processing.
- `TypedFastStateMachine<S, E>` for compile-time state/event domains backed by
  the Fast engine.

The Standard machine defaults to 16 immediate CAS attempts with no wall-clock
budget. Explicitly choose `CasStrategy::LatencyFirst` when a time-bounded retry
window is required; a terminal CAS failure remains distinct from a rejected
transition.

All three entry points keep transition tables immutable after construction and
execute event triggers through CAS-backed state updates. Each also exposes
`diagnose_graph()` for offline reachability analysis; the standard implementation
does not guarantee iteration order, while Fast implementations return code order.

## Why Use It

Use `qubit-state-machine` when you need:

- explicit finite state machine rules built from enum-like state and event types
- immutable transition tables that can be shared across threads
- build-time validation for unknown states and conflicting transitions
- event-driven state updates through `trigger` and `try_trigger`
- success callbacks that observe the old and new state after an update
- simple state tracking for services, jobs, devices, or UI logic
- predictable low-latency path performance through [`FastStateMachine`] with dense
  integer state/event transitions

## Typed Fast State Machine

`TypedFastStateMachine<S, E>` distinguishes state and event types at compile time. Counts come from `DenseCode::VALUES`; `code()` must consistently return the value's index in that complete, unique list. The builder validates the table and runtime entry points check input membership. Decoding uses safe indexing, without a user-defined `from_code` or `transmute`.

```rust
use qubit_state_machine::{DenseCode, TypedFastStateMachine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State { Pending, Running, Done }
impl DenseCode for State {
    const VALUES: &'static [Self] = &[Self::Pending, Self::Running, Self::Done];
    fn code(self) -> u64 { self as u64 }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event { Start, Finish }
impl DenseCode for Event {
    const VALUES: &'static [Self] = &[Self::Start, Self::Finish];
    fn code(self) -> u64 { self as u64 }
}

let machine = TypedFastStateMachine::<State, Event>::builder()
    .initial_state(State::Pending)
    .terminal_state(State::Done)
    .transition(State::Pending, Event::Start, State::Running)
    .transition(State::Running, Event::Finish, State::Done)
    .build().expect("valid job definition");
let state = machine.create_state();
assert_eq!(machine.trigger(&state, Event::Start).expect("start"), State::Running);
assert!(machine.try_trigger(&state, Event::Finish));
assert!(machine.is_terminal_state(state.load()));
```

| Entry point | Intended use and cost |
| --- | --- |
| `StateMachine` | Generic `Copy + Eq + Hash + Debug` states; HashMap lookup and an Arc allocation per candidate update. |
| `FastStateMachine` | Integer protocols with explicit contiguous code ranges, dense lookup, and integer CAS. |
| `TypedFastStateMachine` | Enum lifecycles using the same Fast engine with typed membership checks and no per-transition heap allocation. Measure actual overhead with the benchmarks. |

All three keep immutable rules separate from independent cells. Raw Fast remains a supported integer entry point, not a compatibility shim. Typed cells are created by a machine and expose reads, without setters or raw access; machines with the same state type can share a cell, without machine identity binding.

## Concurrency and Error Boundaries

`trigger` returns detailed errors. `try_trigger` and `try_trigger_with` collapse unknown states, undefined transitions, and exhausted CAS budgets into `false`. Use `trigger` and match errors when business rejection and execution failure require different handling. Typed entry points also reject events omitted from their value table; other errors are preserved through `TypedFastStateMachineError::Raw`.

CAS atomically commits only the state. Conflict retries recompute the event's successor from the newly observed state, without promising the original source or detecting ABA in cycles. Success callbacks run once after commit and may reenter; concurrent callback order is unspecified. A callback may observe a later state, while its arguments and the returned target still describe this call's commit. Callback panics propagate without rollback.

Callers coordinate result publication, hook ordering, and payload synchronization. For example, an executor's `is_done()` means a terminal state was installed, while its result may still be publishing; a nonblocking result read need not succeed yet. A monitor that jointly protects queues, counters, and lifecycle should not be replaced by an independent atomic state machine.

## Installation

The default feature set includes both implementations. Import atomic state
types from their owning crates:

```toml
[dependencies]
qubit-state-machine = "0.9"
qubit-atomic = "0.13"
qubit-fast-cas = "0.3"
```

Use only the standard implementation:

```toml
[dependencies]
qubit-state-machine = { version = "0.9", default-features = false, features = ["standard"] }
qubit-atomic = "0.13"
```

Use only the Fast implementation without pulling in `qubit-cas`:

```toml
[dependencies]
qubit-state-machine = { version = "0.9", default-features = false, features = ["fast"] }
qubit-fast-cas = "0.3"
```

To run the repository's local CI wrapper from a fresh clone, initialize the
version-pinned CI submodule first:

```bash
git submodule update --init --recursive
./ci-check.sh
```

## Quick Start: Job Processing

```rust
use qubit_atomic::AtomicRef;
use qubit_state_machine::StateMachine;

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
        .initial_state(JobState::Queued)
        .terminal_states(&[JobState::Succeeded, JobState::Failed])
        .transition(JobState::Queued, JobEvent::Start, JobState::Running)
        .transition(JobState::Running, JobEvent::Complete, JobState::Succeeded)
        .transition(JobState::Running, JobEvent::Fail, JobState::Failed)
        .build()?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let machine = create_job_machine()?;

    assert!(machine.contains_state(JobState::Running));
    assert!(machine.is_initial_state(JobState::Queued));
    assert!(machine.is_terminal_state(JobState::Succeeded));
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

## Choosing Standard vs Fast

Use `TypedFastStateMachine` for small, fully enumerated enum lifecycles. Use
`StateMachine` when the state set is sparse or assembled at runtime, or when a
fully configured generic CAS executor is needed.

Use `FastStateMachine` when you need low-latency dispatch loops and can model
states/events as dense integer ranges. It trades some ergonomics (explicit bounds,
integer conventions) for constant-time, memory-local transition lookup and tighter
hot-path control.

## Fast State Machine

`FastStateMachine` is for high-throughput loops with dense integer codes.
It validates the full transition table at build time and keeps runtime transition
lookup O(1) with a row-major flat array (`index = state * event_count + event`).

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use qubit_fast_cas::{FastCasPolicy, FastCasState};
use qubit_state_machine::{
    FAST_STATE_MACHINE_DEFAULT_CAS_POLICY,
    FastStateMachine,
};

const QUEUED: u64 = 0;
const RUNNING: u64 = 1;
const SUCCEEDED: u64 = 2;
const FAILED: u64 = 3;
const START: u64 = 0;
const COMPLETE: u64 = 1;
const FAIL: u64 = 2;

let machine = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .terminal_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .build()?;

let tuned = FastStateMachine::builder()
    .state_count(4)
    .event_count(3)
    .initial_state(QUEUED)
    .terminal_states(&[SUCCEEDED, FAILED])
    .transition(QUEUED, START, RUNNING)
    .transition(RUNNING, COMPLETE, SUCCEEDED)
    .transition(RUNNING, FAIL, FAILED)
    .cas_policy(FastCasPolicy::spin(8))
    .build()?;

let state = FastCasState::new(QUEUED);
assert_eq!(machine.trigger(&state, START)?, RUNNING);
let tuned_state = FastCasState::new(RUNNING);
assert_eq!(tuned.trigger(&tuned_state, COMPLETE)?, SUCCEEDED);
assert_eq!(machine.transition_target(QUEUED, START), Some(RUNNING));
assert_eq!(machine.state_count(), 4);
assert_eq!(machine.event_count(), 3);
assert!(machine.is_initial_state(QUEUED));
assert!(machine.is_terminal_state(SUCCEEDED));
assert_eq!(machine.cas_policy(), FAST_STATE_MACHINE_DEFAULT_CAS_POLICY);
assert_eq!(tuned.cas_policy(), FastCasPolicy::spin(8));
# Ok(())
# }
```

`FAST_STATE_MACHINE_DEFAULT_CAS_POLICY` is used when `.cas_policy(...)` is omitted.
Callers can keep defaults during integration and switch to explicit policies later
when contention characteristics require tuning.

## Build-Time Validation

Invalid rules are rejected before a state machine is created.

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
    .initial_state(JobState::Queued)
    .transition(JobState::Queued, JobEvent::Start, JobState::Running)
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
use qubit_atomic::AtomicRef;
use qubit_state_machine::StateMachine;

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
    .initial_state(DoorState::Open)
    .transition(DoorState::Open, DoorEvent::Close, DoorState::Closed)
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
| Add one or more states | `StateMachineBuilder::add_state`, `StateMachineBuilder::add_states` |
| Configure fast state/event space | `FastStateMachineBuilder::state_count`, `FastStateMachineBuilder::event_count` |
| Mark the unique initial and terminal states | `initial_state`, `terminal_state`, `terminal_states` |
| Add transition rules | `transition`, `transition_value`, `Transition` |
| Query transition targets without changing state | `transition_target` |
| Apply events and get detailed errors | `trigger`, `trigger_with`, `StateMachineError` |
| Apply events without handling errors | `try_trigger`, `try_trigger_with` |
| Store shared mutable state | `qubit_atomic::AtomicRef` or `qubit_fast_cas::FastCasState` |

For the complete job-lifecycle walkthrough, retry-policy guidance, error
handling, and troubleshooting steps, read the [English user guide](doc/user_guide.md)
or the [中文用户手册](doc/user_guide.zh_CN.md). The generated [API Reference](https://docs.rs/qubit-state-machine)
contains the complete public API documentation.

## Core API At A Glance

| Type | Purpose |
| --- | --- |
| `Transition` | Immutable value describing `source --event--> target`. |
| `FastStateMachine` | Dense integer-coded transition machine for high-throughput scenarios. |
| `FastStateMachineBuilder` | Builder for state/event code counts, transition table, and CAS policy. |
| `FastStateMachineError` | Runtime error from fast transition execution. |
| `FastStateMachineBuildError` | Build-time validation error for fast transition table configuration. |
| `StateMachineBuilder` | Builder for states, one initial state, terminal states, and transitions. |
| `StateMachine` | Immutable, validated transition table used to query and trigger events. |
| `StateMachineBuildError` | Validation error returned while building invalid rule sets. |
| `StateMachineError` | Runtime error returned when an event cannot be applied. |

## Project Scope

- `qubit-state-machine` is intended for simple finite state machines, not a full
  workflow engine.
- State and event types should be small enum-like values implementing
  `Copy + Eq + Hash + Debug`.
- Fast state machines require dense `u64` state/event codes in `[0, state_count)` and
  `[0, event_count)` and a complete table budget of `state_count * event_count`.
- Rule definitions become immutable after `StateMachineBuilder::build`.
- Standard event triggering uses `AtomicRef<S>` with CAS execution via `qubit-cas`.
- Success callbacks are executed only after CAS transition succeeds.
- `FastStateMachine` uses compact state/event integer codes and a flat transition
  table for predictable O(1) transition lookup.

## Rust Version

This crate uses Rust 2024 edition and requires Rust 1.94 or newer.

## Dependencies

Runtime dependencies are feature-scoped:

- `thiserror` provides concrete error implementations.
- `standard` enables `qubit-atomic` and `qubit-cas`.
- `fast` enables only `qubit-fast-cas`.

The default feature set enables `standard` and `fast`.

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./align-ci.sh` to format code and
`./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-state-machine](https://github.com/qubit-ltd/rs-state-machine)
