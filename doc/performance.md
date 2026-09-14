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

## Reproduce selected scenarios

From the crate root, run each command three separate times on an otherwise
idle machine. Criterion uses 20 samples, a 300 ms warm-up, and a 1 s
measurement period per scenario. The `task_lifecycle` measurement applies
`Start` and `CompleteSucceeded` to one cell; its cell construction is outside
the measured operation. Both commands use all features so all four drivers
are present.

```bash
cargo bench --bench state_machine_benchmarks --all-features transition_target -- --noplot
cargo bench --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

## Example results

Measured on 2026-09-14 with the benchmark implementation at revision
`f2c4a7d`: Rust 1.94.0, Ubuntu 24.04.4 LTS, Linux 7.0.0-30-generic,
Intel Core i5-9600K (six cores, 3.70 GHz). The values below are Criterion
mean point estimates in nanoseconds per operation from three separate
invocations after a preliminary warm-up run. Lower is faster for these
specific workloads.

| Driver | Lookup run 1 | Run 2 | Run 3 | Lifecycle run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Direct AtomicU64 baseline | 4.783 | 4.780 | 4.816 | 18.016 | 18.101 | 18.065 |
| Raw Fast | 5.146 | 5.186 | 5.193 | 24.336 | 24.224 | 24.233 |
| Typed Fast | 6.371 | 6.369 | 6.424 | 25.184 | 24.839 | 25.601 |
| Standard | 18.659 | 19.621 | 19.426 | 1259.550 | 1234.473 | 1267.393 |

These measurements compare only lookup and an uncontended two-transition
lifecycle. The Standard path updates `AtomicRef` values, whereas the other
drivers use integer CAS; the direct baseline is not a replacement for rule
validation or typed errors. The table does not establish performance under
contention, with callbacks, or on another machine. Run the remaining benchmark
scenarios and measure your own workload before choosing an implementation.
