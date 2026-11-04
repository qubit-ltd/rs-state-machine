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
`Start` and `CompleteSucceeded` to one cell. Cell construction and final
destruction are outside the measured interval. Allocations and releases
performed inside transitions remain included. Both commands use all features
so all four drivers are present.

Each timed lifecycle sample unwraps both transition results, checks that
`Start` returns target 1 and `CompleteSucceeded` returns target 2, then loads
the cell and checks that it contains 2. Result unwrapping, target assertions,
the final load, and its assertion are inside the timed closure. Cell
construction and final destruction remain outside it. Existing lifecycle
tables and archived outputs are preserved as historical records: their source
did not include these per-sample checks, so they describe the earlier
measurement boundary and are not data collected with the checks described
here. This does not establish that either set of results is wrong or that the
checked method is faster.

```bash
cargo bench --bench state_machine_benchmarks --all-features transition_target -- --noplot
cargo bench --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

## Shared-toggle contention statistics

Each `shared_toggle_t{workers}` Criterion invocation also writes one raw
`rsm_shared_toggle_v1` record to stderr after all of its batches finish. Save
the records separately for each run; Criterion calibration and warm-up invoke
the same benchmark closure, so the log includes those calls as well as measured
sampling calls. These are per-invocation counters, not Criterion samples or
confidence estimates.

```bash
cargo bench --locked --bench state_machine_benchmarks --all-features shared_toggle -- --noplot 2>target/shared-toggle-run1.log
```

The tab-separated record reports `batches`, total `calls`, each outcome count,
and summed `elapsed_ns`. Calls are `batches * workers * 4096`; each call is
classified as committed, rejected, conflict, or unexpected. The rates are
`calls_per_second = calls / (elapsed_ns / 1_000_000_000)`,
`commits_per_second = committed / (elapsed_ns / 1_000_000_000)`, and
`conflict_ratio = conflicts / calls`. A zero elapsed time makes either rate
`unavailable`; zero calls makes the conflict ratio `unavailable`.

The returned Criterion duration sums only the measured worker intervals from
`measure_shared_toggle`. Counter aggregation and stderr formatting happen
after each interval and before the duration is returned, so log I/O is outside
that duration. Criterion's own calibration and warm-up calls still produce
records. To combine records, sum counts and elapsed time first, then recompute
the ratios; do not average the printed rates. Keep the original logs and record
the source revision, toolchain, CPU, and operating system for each run.

## Example results

Historical measurements from 2026-09-14 use the benchmark implementation at revision
`f2c4a7d`: Rust 1.94.0, Ubuntu 24.04.4 LTS, Linux 7.0.0-30-generic,
Intel Core i5-9600K (six cores, 3.70 GHz). The values below are Criterion
mean point estimates in nanoseconds per operation from three separate
invocations after a preliminary warm-up run. Lower is faster for these
specific workloads. The historical `task_lifecycle` measurements include
final cell destruction in the measured interval; their lifecycle values must
not be directly compared with measurements using the corrected boundary. The
lookup measurements retain their original measurement boundary.

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

## Corrected task-lifecycle measurements (2026-11-01 UTC)

Three independent runs were collected with Criterion 0.8.2 using
`cargo bench --locked --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot`.
Each cell is the Criterion mean point estimate in ns/op; each invocation used 20
samples, a 300 ms warm-up, and a 1 s measurement period. The results are
measurements, not a performance threshold.

| Driver | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Direct AtomicU64 baseline | 13.704 | 14.849 | 15.104 |
| Raw Fast | 20.960 | 23.509 | 21.063 |
| Typed Fast | 22.127 | 27.574 | 22.290 |
| Standard | 1081.9 | 1277.4 | 1097.5 |

Environment: Rust 1.94.0 (`4a4ef493e3a1488c6e321570238084b38948f6db`), Linux
7.0.0-38-generic x86_64, Intel Core i5-9600K (6 cores, 3.70 GHz). The source
was an uncommitted working tree based on `e58629fb8216faa30c2749a194a1541b258ceb3a`;
the pre-measurement diff covered 10 files (421 insertions, 36 deletions).
`Cargo.lock` SHA-256: `e150c902a109cd0be9bfdc470acc8d0c00edd37b9860184c5b389abd7470019f`.
The three complete Criterion outputs were saved as
`target/rsm-lifecycle-054a6b95-run{1,2,3}.log` during this run. Historical
lifecycle values above use a different timing boundary and are not directly
comparable. This historical record has no recoverable source snapshot or
persistent raw output, so it cannot be independently verified. The `target/`
paths describe local files from that run; they are not a persistent archive.

## Archived task-lifecycle measurements (2026-11-01 UTC)

This new set uses the source at commit
`9e8a7ed176659358f11be6186c0f6f2f475cc8a3`. Each value is the Criterion
`mean.point_estimate` in ns/op, read from the archived estimates JSON for that
run. Criterion used 20 samples, a 300 ms warm-up, and a 1 s measurement period.
Cell construction and final destruction are outside the measured interval;
allocations and releases performed inside transitions remain included.

| Driver | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Direct AtomicU64 baseline | 16.727 | 26.468 | 13.762 |
| Raw Fast | 21.296 | 31.774 | 22.467 |
| Typed Fast | 23.758 | 24.779 | 23.028 |
| Standard | 1359.656 | 1676.236 | 1129.600 |

The persistent evidence is in
[`performance-evidence/lifecycle-rsm001-20261101T122148Z/`](performance-evidence/lifecycle-rsm001-20261101T122148Z/):
[metadata](performance-evidence/lifecycle-rsm001-20261101T122148Z/metadata.md),
[run 1 output](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-1.txt),
[run 2 output](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-2.txt),
[run 3 output](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-3.txt),
[run 1 estimates](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-1-estimates.json),
[run 2 estimates](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-2-estimates.json),
[run 3 estimates](performance-evidence/lifecycle-rsm001-20261101T122148Z/run-3-estimates.json), and
[SHA-256 manifest](performance-evidence/lifecycle-rsm001-20261101T122148Z/checksums.sha256).
An earlier wrapper attempt whose exit code could not be recorded is retained
separately in
[`performance-evidence/lifecycle-rsm001/`](performance-evidence/lifecycle-rsm001/)
and is not included in this table.

To repeat the measurement, check out the commit in the metadata, use the listed
Rust and Cargo toolchain and environment, then run this command three separate
times from the crate root and retain each complete output:

```bash
cargo bench --locked --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

Verify the source input hashes against the manifest before and after the runs.
Matching the source and method makes the measurement reproducible; it does not
guarantee the same values under different machine load or environments. These
measurements are not a correctness or performance threshold.
