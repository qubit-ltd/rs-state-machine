# State-machine performance measurements

The benchmark compares the direct AtomicU64 baseline, Standard, Raw Fast, and
Typed Fast implementations. Lookup, single-thread transitions, task lifecycle,
and prepared-cell batch races are separate scenarios.
Worker readiness is synchronized before timing; thread and cell construction are
outside the measured interval.

Standard uses 16 immediate attempts without time budgets. Fast and Typed Fast use
the explicit `spin(16)` policy. Counts distinguish calls, committed transitions,
business rejections, exhausted CAS budgets, and unexpected failures.

Results are machine-specific. Each result must record the Rust toolchain, CPU,
operating system, source revision, and three independent benchmark runs. Batch
success throughput and total call throughput are different quantities. Same-value
Fast CAS and Standard `AtomicRef` identity updates are not interchangeable
measurements.
