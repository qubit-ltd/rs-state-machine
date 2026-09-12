# Qubit State Machine User Guide

[中文版](user_guide.zh_CN.md) · Applies to version 0.9

## Purpose and Audience

This guide is for Rust developers modeling a job, service, device, or other
finite lifecycle with explicit allowed transitions. It covers the public
`StateMachine`, `FastStateMachine`, and `TypedFastStateMachine` APIs. It does
not turn this crate into a workflow scheduler or a cross-resource transaction.

## Conceptual Model

An immutable machine owns the registered states, the unique initial state,
terminal-state markers, and transition rules. A separately owned state cell
holds one object's current state. Multiple objects can share the same machine
while keeping independent state cells.

The Standard machine stores enum-like values in `qubit_atomic::AtomicRef` and
uses synchronous `qubit-cas` execution. The Fast machine stores dense `u64`
codes in `qubit_fast_cas::FastCasState`; the typed Fast machine checks those
codes against each type's `DenseCode::VALUES`.

## Scenario: Start and Audit a Job

Suppose a worker must move a job from `Queued` to `Running` and record an audit
entry only after the state commit succeeds. A repeated `Start` must be
reported as a rejected transition.

## Installation and Minimal Configuration

The Standard-only setup keeps the dependency surface small:

```toml
[dependencies]
qubit-state-machine = { version = "0.9", default-features = false, features = ["standard"] }
qubit-atomic = "0.13"
qubit-cas = "0.9"
```

Use Rust 1.94 or newer. The default feature set enables both `standard` and
`fast`.

## Core Workflow

Define small `Copy + Eq + Hash + Debug` state and event types, register the
states, select exactly one initial state, add transitions, and build the
immutable table. Create a state cell for each job and use `trigger_with` when
the successful transition needs an audit callback:

```rust
use qubit_atomic::AtomicRef;
use qubit_cas::CasExecutor;
use qubit_state_machine::StateMachine;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JobState { Queued, Running }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum JobEvent { Start }

let executor = CasExecutor::builder().max_attempts(1).no_delay()
    .build().expect("valid retry settings");
let machine = StateMachine::builder()
    .add_states(&[JobState::Queued, JobState::Running])
    .initial_state(JobState::Queued)
    .transition(JobState::Queued, JobEvent::Start, JobState::Running)
    .cas_executor(executor)
    .build().expect("valid transition table");
let state = AtomicRef::from_value(JobState::Queued);
let mut audit = Vec::new();

machine.trigger_with(&state, JobEvent::Start, |old, next| {
    audit.push((old, next));
}).expect("start transition commits");
assert_eq!(*state.load(), JobState::Running);
assert_eq!(audit, vec![(JobState::Queued, JobState::Running)]);
assert!(!machine.try_trigger(&state, JobEvent::Start));
```

The observable result is one committed state change and one audit entry. Use
`trigger` or `trigger_with` when the caller must distinguish a rejected
business transition from a CAS execution failure; use `try_trigger` or
`try_trigger_with` when all failures may be collapsed to `false`.

## Advanced Usage

`cas_executor` injects a validated synchronous executor. `cas_strategy` selects
`LatencyFirst`, `ContentionBackoff`, or `ReliabilityFirst`; the last call that
sets the executor or strategy wins. The default is 16 immediate attempts with
no operation or total wall-clock budget. Inspect the installed executor with
`machine.cas_executor()`, including `max_attempts()`,
`max_operation_elapsed()`, and `max_total_elapsed()`.

Use `FastStateMachine` when states and events are dense `u64` codes and the
flat transition table is appropriate. Use `TypedFastStateMachine<S, E>` when
compile-time state and event types are useful; its `DenseCode::VALUES` list
must be complete and unique. All three machine types provide
`diagnose_graph()` for offline reachability analysis; it does not change build
validity.

## Errors and Diagnostics

Build errors cover missing definitions, conflicting duplicate transitions, an invalid initial state,
and transitions that violate the registered state or terminal-state rules.
At runtime, Standard returns `UnknownState`, `UnknownTransition`, or
`CasFailure`; the latter retains the CAS failure kind and attempt count,
including exhausted conflicts or soft budgets. Fast returns the corresponding
`FastStateMachineError` variants, including `CasConflict` when its retry policy
is exhausted. Typed Fast additionally reports invalid codebook membership.

`trigger_with` invokes its callback once after a successful commit, including a
self-transition. A callback panic propagates and does not roll back the state.
Concurrent callback order is unspecified, and a fresh read inside a callback
may see a later commit; use the callback's `old` and `new` arguments for the
audit record.

## Troubleshooting

- If `build` fails, check that the initial state is registered. Repeated
  `initial_state(...)` calls are allowed and the last value wins; terminal
  states have no outgoing transitions, and every transition endpoint
  is registered.
- If triggering fails, match the detailed error before deciding whether a CAS
  conflict or a business rejection is retryable.
- If latency increases, inspect synchronous retry delays and contention; use
  a bounded strategy and measure the workload rather than making retries
  unbounded.
- For Fast machines, verify every code is within the configured state/event
  range. For typed Fast machines, verify `code()` matches the index in
  `DenseCode::VALUES`.

## Limitations and Best Practices

Rules are immutable after `build`, but each job still needs its own state cell.
CAS only atomically commits the state; result delivery, hook ordering, and
business payload synchronization remain the caller's responsibility. The
synchronous trigger path does not provide async execution or external
transaction coordination. This crate is a finite state machine, not a complete
workflow engine.

## Further Reading

- [README](../README.md)
- [中文用户手册](user_guide.zh_CN.md)
- [API Reference](https://docs.rs/qubit-state-machine)
