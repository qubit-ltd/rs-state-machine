# RSM-003 benchmark provenance and evidence

No candidate was measured because source inspection found no semantically equivalent optimization. Three clean current-path samples were collected after an explicitly authorized quiet window. They do not compare old and new implementations or establish improvement.

Environment and benchmark construction are recorded in `../../review-rsm003-evidence.md`. The legacy command was:

```text
cargo bench --locked --bench state_machine_benchmarks --all-features -- --noplot 'standard_state_machine/(self_transition|task_lifecycle|business_rejection)'
```

Legacy runs 1–4 are `run-1.txt` through `run-4.txt`; they exited 0 and reported all selected names, but used the unisolated worktree target while host build activity was later observed. Run 1 has no preserved estimates. All 12 JSON files copied under `superseded-run-2-duplicate-of-preexisting-cache/` compare byte-for-byte with the saved pre-existing Criterion cache under `pre-existing-criterion-base-excluded/`; run 2's copied estimates are duplicates, not its actual sample. Runs 3 and 4 have copied Criterion `new` artifacts, but the unisolated target and host activity mean they are diagnostic only. None count as clean samples.

Isolated-target diagnostics 1–3 are under `isolated/`. Their estimate JSON hashes match the exact source files in `/tmp/qubit-rsm003-isolated-target-20261103/criterion/standard_state_machine/{scenario}/new/`. Concurrent Cargo/rustc work was active during that earlier window, so those three are contaminated and excluded. Their raw logs are `isolated/run-1.txt` through `run-3.txt`, with estimates alongside.

Clean isolated-target runs 5–7 are under `isolated/run-5.txt` through `run-7.txt`; each has its immediately copied complete `new/` artifacts under the matching `run-N-estimates/standard_state_machine/{scenario}/` directory. The copied estimate hashes were checked against the live Criterion target after each run. Immediately before and after each run, `uptime` and `ps -C cargo -C rustc -C cargo-llvm-cov -o pid,comm,args --no-headers` were checked; no matching Cargo/rustc process appeared. Pre-run 1/5/15-minute loads were 0.66/0.32/0.16, 0.67/0.35/0.17, and 0.61/0.36/0.18. Post-run loads were 0.73/0.36/0.17, 0.66/0.37/0.18, and 0.54/0.36/0.18.

The output's `time` estimate corresponds to `slope` in `estimates.json`. Criterion's `change` lines compare its baseline run in this isolated target and are excluded from analysis because there is no candidate implementation. See the review file for run 5–7 estimates and interpretation. T3 measurement is complete for the current path; no A/B or performance improvement claim is made.

`checksums.sha256` covers every evidence file other than the checksum file itself. Recreate from this directory with:

```text
find . -type f ! -name checksums.sha256 -print0 | sort -z | xargs -0 sha256sum > checksums.sha256
```
