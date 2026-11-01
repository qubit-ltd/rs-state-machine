# RSM-001 failed wrapper attempt

Command attempted:

```bash
cargo bench --locked --bench state_machine_benchmarks --all-features task_lifecycle -- --noplot > doc/performance-evidence/lifecycle-rsm001/run-1.txt 2>&1
```

The retained `run-1.txt` contains output for all four `task_lifecycle` drivers. After the benchmark command returned, the outer zsh command tried to store its exit code in the read-only special variable `status` and failed with `zsh:1: read-only variable: status`. The outer shell returned 1, but the Cargo command's exit code was not reliably recorded. This output is preserved as failure evidence and was not used in the successful results table.

A fresh, complete three-run series was then collected without overwriting this file at `../lifecycle-rsm001-20261101T122148Z/`; each Cargo command returned exit code 0.
