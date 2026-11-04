# Qubit State Machine (`rs-state-machine`)

[![Rust CI](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-state-machine/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-state-machine/coverage-badge.json)](https://qubit-ltd.github.io/rs-state-machine/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-state-machine.svg?color=blue)](https://crates.io/crates/qubit-state-machine)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

`qubit-state-machine` provides immutable transition rules and independent atomic state cells for finite task, service, and device lifecycles. It makes rejected events explicit while leaving scheduling and result storage to the caller.

## Installation

The default feature set includes both implementations. Import atomic state
types from their owning crates:

```toml
[dependencies]
qubit-state-machine = "0.11"
qubit-atomic = "0.17"
qubit-fast-cas = "0.3"
```

Use only the standard implementation:

```toml
[dependencies]
qubit-state-machine = { version = "0.11", default-features = false, features = ["standard"] }
qubit-atomic = "0.17"
```

Use only the Fast implementation without pulling in `qubit-cas`:

```toml
[dependencies]
qubit-state-machine = { version = "0.11", default-features = false, features = ["fast"] }
qubit-fast-cas = "0.3"
```

Run the local CI wrapper from the crate root:

```bash
./.infra/bin/ci-check.sh
```

The wrapper bootstraps the shared rs-infra tools and caches their sources and
binaries. A first run may need network access and build the tool manager;
initializing a CI submodule is not required.

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

## Why Use It

Use `qubit-state-machine` to define explicit lifecycle rules, share immutable tables across threads, validate rules at build time, and apply events with detailed or boolean results. Choose among generic Standard, dense integer Fast, and typed Fast entry points after the first working example above.

| Entry point | Intended use and cost |
| --- | --- |
| `StateMachine` | Generic `Copy + Eq + Hash + Debug` states; suitable for sparse values, existing `AtomicRef` cells, or full CAS configuration. HashMap lookup and an Arc allocation per candidate update. |
| `FastStateMachine` | Integer protocols with explicit contiguous code ranges, dense lookup, and integer CAS. Measure it against your workload. |
| `TypedFastStateMachine` | Enum lifecycles using the same Fast engine with typed membership checks and no per-transition heap allocation. Measure actual overhead with the benchmarks. |

All three entry points keep immutable rules separate from independent cells and offer `diagnose_graph()` for offline reachability analysis. The Standard result order is unspecified; Fast results use code order.

## Choosing Standard vs Fast

Use `TypedFastStateMachine` for small, fully enumerated enum lifecycles. Use
`StateMachine` when the state set is sparse or assembled at runtime, or when a
fully configured generic CAS executor is needed.

Use `FastStateMachine` when you need low-latency dispatch loops and can model
states/events as dense integer ranges. It trades some ergonomics (explicit bounds,
integer conventions) for constant-time, memory-local transition lookup and tighter
hot-path control. See the [benchmark method and measured examples](doc/performance.md)
before drawing performance conclusions for your workload.

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

Raw Fast remains a supported integer entry point, not a compatibility shim. Typed cells are created by a machine and expose reads, without setters or raw access; machines with the same state type can share a cell, without machine identity binding.

## Fast State Machine

`FastStateMachine` uses dense integer codes and a flat table. The default table
budget is `1_048_576` cells (`state_count * event_count`); adjust it with
`max_table_cells(limit)` when the memory cost is understood. Exceeding the
limit returns `FastStateMachineBuildError::TransitionTableLimitExceeded`.
It validates the full transition table at build time and keeps runtime transition
lookup O(1) with a row-major flat array (`index = state * event_count + event`).

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    Ok(())
}
```

`FAST_STATE_MACHINE_DEFAULT_CAS_POLICY` is used when `.cas_policy(...)` is omitted.
Callers can keep defaults during integration and switch to explicit policies later
when contention characteristics require tuning.

## Concurrency and Error Boundaries

`trigger` returns detailed errors; `try_trigger` and `try_trigger_with` collapse unknown states, undefined transitions, and exhausted CAS budgets to `false`. Use the detailed API when business rejection and execution failure need different handling.

CAS commits only the state. Callers coordinate result publication and related payloads; a terminal state does not guarantee that a result is already readable. Full retry, callback, and concurrency boundaries are in [Errors and Diagnostics](doc/user_guide.md#errors-and-diagnostics) and [Limitations and Best Practices](doc/user_guide.md#limitations-and-best-practices).

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
./.infra/bin/ci-check.sh

# Check code coverage
./.infra/bin/coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public
API documentation and tests current, and run `./.infra/bin/align-ci.sh` to format code and
`./.infra/bin/ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-state-machine](https://github.com/qubit-ltd/rs-state-machine)
