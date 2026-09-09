# Migrating to qubit-state-machine 0.9

Version 0.9 changes the generic `StateMachine` default CAS executor. It now
allows 16 immediate attempts and has no operation or total wall-clock budget.
This avoids rejecting an otherwise valid transition because the scheduler
delayed a retry. Strong contention can still exhaust the attempt limit and is
reported as a CAS failure.

Applications that require the 0.8 latency-bounded preset must opt in explicitly:

```rust
use qubit_cas::CasStrategy;

let machine = StateMachine::<u8, u8>::builder()
    .add_states(&[0, 1])
    .initial_state(0)
    .transition(0, 0, 1)
    .cas_strategy(CasStrategy::LatencyFirst)
    .build()?;
```

The `trigger` APIs still distinguish rejected transitions from terminal CAS
failures. `try_trigger` continues to collapse both categories to `false`.
Typed Fast, Raw Fast, state-cell ownership, callbacks, and terminal-state rules
are unchanged. `is_done()` in downstream executors still means that the terminal
state was installed; a result may become readable immediately afterwards.
