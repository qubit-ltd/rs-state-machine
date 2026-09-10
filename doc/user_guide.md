# Qubit State Machine User Guide

Applies to 0.9. [中文版](user_guide.zh_CN.md). This guide is for Rust applications
that need explicit job lifecycle rules.

## Model and setup

Transition tables are immutable after construction. Standard machines store
small enum-like states in AtomicRef; Fast machines use compact integer states.
Both require one initial state and reject outgoing transitions from terminal
states. Each job owns a state cell independently of the shared rule table.

The Standard builder defaults to 16 immediate CAS attempts without operation or
total wall-clock budgets. Select `CasStrategy::LatencyFirst` explicitly when a
time-bounded retry window is part of the application contract.

```toml
[dependencies]
qubit-state-machine = { version = "0.9", default-features = false, features = ["standard"] }
qubit-atomic = "0.13"
qubit-cas = "0.14"
```

## Start and audit a job

The job starts Queued. Start commits Running, then records an audit event. A
second Start fails because no such transition exists from Running.

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
machine.trigger_with(&state, JobEvent::Start, |old, next| audit.push((old, next)))
    .expect("start transition commits");
assert_eq!(*state.load(), JobState::Running);
assert_eq!(audit, vec![(JobState::Queued, JobState::Running)]);
assert!(!machine.try_trigger(&state, JobEvent::Start));
```

## Retry configuration

`cas_executor` injects a validated executor; `cas_strategy` selects LatencyFirst,
ContentionBackoff, or ReliabilityFirst. Both replace the entire executor, with
the last call winning. ContentionBackoff is fixed exponential backoff plus jitter,
not an adaptive controller. The Standard default uses 16 immediate attempts and
no operation or total wall-clock budget; choose `LatencyFirst` explicitly for a
time-bounded policy.

After building, inspect the installed limits through `machine.cas_executor()`
using `max_attempts()`, `max_operation_elapsed()`, and `max_total_elapsed()`.
Backoff is configured on the builder and has no public getter. A custom executor
is not assigned a preset strategy name.

Custom builders support max_attempts, max_operation_elapsed, max_total_elapsed,
and retry delays. Soft budgets gate later attempts without revoking a committed
success. Synchronous triggering ignores attempt_timeout/flow_timeout and blocks
during backoff. Use rs-cas directly when async execution or hooks are needed.
Typed Fast errors expose `is_unknown_transition()` and `is_cas_conflict()`.
`diagnose_graph()` performs offline reachability analysis without changing build
validity.

## Errors and side effects

UnknownState/UnknownTransition represent invalid business transitions. CasFailure
retains the CAS kind and attempt count, including exhausted conflicts or budgets.
try_trigger compresses errors to false; use trigger when diagnostics matter.

trigger_with invokes its callback once after a successful commit, including
self-transitions. A callback panic does not roll back state. Concurrent callbacks
have no global order and a fresh state read may observe a newer commit; use the
supplied old/new arguments for that transition's audit record.

## Fast mode and troubleshooting

Use default-features=false, features=["fast"] and qubit-fast-cas 0.3 for fast-only
consumers, without qubit-cas or qubit-atomic. Integer codes must stay within the
configured state/event ranges.

- Build failure: check initial state, registrations, terminal edges, and duplicates.
- Trigger failure: distinguish business errors from CasFailure before retrying.
- High latency: inspect synchronous backoff and hot contention; measure presets
  rather than making retries unbounded.

This crate is not a cross-resource transaction or full workflow scheduler. See
the [README](../README.md), [API](https://docs.rs/qubit-state-machine), and
[migration note](migration-0.9.md).
