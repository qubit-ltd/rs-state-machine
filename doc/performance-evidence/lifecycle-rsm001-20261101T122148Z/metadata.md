# RSM-001 lifecycle benchmark evidence

## Measurement identity

- Collected: 2026-11-01 UTC; runs started at 12:21:48Z, 12:22:05Z, and 12:22:18Z.
- Source commit: `9e8a7ed176659358f11be6186c0f6f2f475cc8a3`.
- Source status before and after: no diff in `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/`, or `benches/`; no untracked files under `src/` or `benches/`.
- Source input SHA-256 values are listed in `checksums.sha256`. The same inputs were hashed before and after measurement; all hashes matched.
- Benchmark: Criterion group drivers `atomic_baseline` (Direct AtomicU64 baseline), `fast_state_machine` (Raw Fast), `typed_fast_state_machine` (Typed Fast), and `standard_state_machine` (Standard); scenario `task_lifecycle`.
- Features: all features enabled.
- Criterion: 0.8.2 from `Cargo.lock`; sample size 20, warm-up 300 ms, measurement time 1 s. There were no custom Criterion settings for this run.

## Environment

- `rustc -Vv`: rustc 1.94.0 (`4a4ef493e3a1488c6e321570238084b38948f6db`), host `x86_64-unknown-linux-gnu`, LLVM 21.1.8.
- `cargo -V`: cargo 1.94.0 (`85eff7c80`, 2026-01-15).
- Kernel: Linux `7.0.0-38-generic` (`x86_64`).
- CPU: Intel Core i5-9600K, 6 cores, 3.70 GHz base frequency; the reported CPU frequency scaling was 94% at collection time.
- Cargo environment overrides (`CARGO_HOME`, `CARGO_TARGET_DIR`, `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `CARGO_BUILD_TARGET`, and `CARGO_PROFILE_BENCH_*`) were not set. There was no repository `.cargo/config.toml`. The user Cargo config only set debug information for dev and test profiles; no bench profile, target, source replacement, or build flags were configured. No extra `RUSTFLAGS` were used.
- Criterion reported that Gnuplot was unavailable and used its Plotters backend; plotting was disabled by `--noplot`.

## Commands and outcomes

Each run used this command from the crate root, serially, with stdout and stderr captured together in its corresponding `run-N.txt` file:

```bash
cargo bench --locked --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot
```

| Run | Start time (UTC) | Command exit code | Complete output | Mean estimates |
| --- | --- | ---: | --- | --- |
| 1 | 2026-11-01 12:21:48 | 0 | `run-1.txt` | `run-1-estimates.json` |
| 2 | 2026-11-01 12:22:05 | 0 | `run-2.txt` | `run-2-estimates.json` |
| 3 | 2026-11-01 12:22:18 | 0 | `run-3.txt` | `run-3-estimates.json` |

Each estimates JSON file contains the raw Criterion estimate objects captured immediately after that run from `target/criterion/<driver>/task_lifecycle/new/estimates.json`, keyed by the driver/scenario path. The displayed table values are the corresponding `mean.point_estimate`, expressed in ns/op and rounded to three decimal places:

| Driver | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Direct AtomicU64 baseline | 16.726619388913903 | 26.467555963262186 | 13.762051223643658 |
| Raw Fast | 21.29601564437318 | 31.77414619532771 | 22.46744284269159 |
| Typed Fast | 23.75845059184362 | 24.779463662598918 | 23.027923067285293 |
| Standard | 1359.6559919271208 | 1676.236224034847 | 1129.599963344524 |

## Timing boundary and reproduction

`task_lifecycle` applies `Start` and `CompleteSucceeded` to one state cell. Cell construction and final destruction are outside the timed interval. Allocations and releases performed within either transition remain included. All four drivers use the same benchmark scenario and sampling settings.

To reproduce the method, check out the source commit above, use Rust and Cargo 1.94.0 and the listed environment, verify source input hashes before and after, then run the command above three separate times and retain each complete output. The same source and procedure support a repeatable measurement; they do not guarantee identical values under different machine load or environments. These values are not a performance threshold.

An earlier wrapper attempt is recorded separately at `../lifecycle-rsm001/`. Its complete Criterion output is retained, but its outer shell could not capture a reliable Cargo exit code; it is not part of these three successful runs or their results.
