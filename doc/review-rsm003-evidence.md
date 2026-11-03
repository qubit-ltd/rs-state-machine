# RSM-003: Standard precheck review

## Decision

Retain the Standard precheck. No production edit is justified: skipping it is not semantically equivalent for all public `CasExecutor` configurations, and it also changes behavior under a concurrent state change between the precheck load and the executor's attempt load. The prescribed gates therefore fail before a candidate A/B is valid. No performance improvement is claimed.

## Scope and provenance

- Baseline source revision: `39d7c211d2ab455bdd5b1dc9540ca354cfb6c4df` (`codex/rsm-implementation-20261103`).
- Plan/design: `doc/implementation-plan-20261103T174931857Z-cf33135f.md`, task T3; `doc/design-20261103T174931857Z-cf33135f.md`, RSM-003.
- Locked dependencies: `qubit-cas 0.12.1`, checksum `565f96056ccfb0447794cce38f032533c59e02aae28f9dfd18e6e1f6e62f6004`; `qubit-retry 0.26.0`, checksum `84286aee1cde9a49e1d050eb1792c76919a522081b83c76167a1299ea8a46d23`.
- Toolchain: `rustc 1.94.0 (4a4ef493e 2026-03-02)`, host `x86_64-unknown-linux-gnu`; active toolchain `1.94.0-x86_64-unknown-linux-gnu` from the repository's `rust-toolchain.toml`.
- Host: Linux `6.8.0-147-generic` x86_64; Intel Xeon E5-2680 v4 @ 2.40 GHz, 2 sockets, 14 cores/socket, 2 threads/core (56 online CPUs), 70 MiB reported L3.
- T3 working-tree edits: benchmark driver adds `business_rejection`; this evidence file and the `doc/performance-evidence/rsm003/` directory. No production implementation or contract-test changes.
- The working tree also contains unrelated-to-T3 concurrent T1/T2 edits; those are not included in this T3 diff. The two design/plan documents are inputs and must remain untracked/unmodified/uncommitted.

## Dependency and call-order inspection

The current `StateMachine::change_state` in `src/standard/state_machine.rs` does the following:

1. Loads `AtomicRef<S>` once and calls `next_state` against that snapshot.
2. Returns `UnknownState` or `UnknownTransition` immediately if that lookup fails.
3. Otherwise calls `CasExecutor::execute_result`.
4. On every executor attempt, `qubit-cas 0.12.1`'s `run_sync_attempt` loads `AtomicRef<S>` again, invokes the operation closure, and applies its `CasDecision` to that loaded allocation. The closure calls `next_state` again and returns `Update` or `Abort`.
5. A changed pointer causes the CAS update to fail; the retry adapter may invoke the closure again, which recomputes the transition from the newly loaded state. An abort is terminal. `trigger_with` invokes its callback only after `change_state` has returned success.

Sources inspected in the local registry: `qubit-cas-0.12.1/src/executor/cas_executor/sync_execution.rs` (`execute_result`, `run_sync_attempt`), `src/executor/cas_executor/decision.rs` (`apply_decision`), `qubit-cas-0.12.1/src/executor/cas_builder.rs` (zero `max_attempts` rejected during build), and `qubit-retry-0.26.0/src/executor/retry.rs` plus `src/executor/internal/flow/retry_flow_controller.rs` (admission before each operation).

The first executor attempt is not guaranteed to invoke the closure: retry controller `before_attempt` checks flow timeout, cancellation, and continuation limits before `commit_attempt` and before `operation()`. Zero `max_attempts` is rejected by builder validation, but zero operation-time or total-time budget is a valid configuration and can exhaust admission before any attempt. The current precheck consequently returns the transition error before executor budget exhaustion. Removing it would change externally visible error priority.

There is a second independent semantic difference: if the first snapshot has no transition but another thread installs a state that does, current behavior returns the precheck error; moving the first validation entirely into the executor may apply the event to the later snapshot. Conversely, if the precheck passes but a concurrent change occurs before the executor load, the existing closure correctly revalidates and either applies the transition for the new snapshot or aborts. This two-load behavior is deliberate and cannot be replaced by the initial result without changing retry/recomputation semantics.

## Performance method and status

The existing Criterion driver already times Standard `self_transition` (a successful self-conversion) and `task_lifecycle` with fixture construction outside the timed closure. T3 adds `standard_state_machine/business_rejection`; its fixture is created outside timing, and the rejected event has no transition from the task fixture's initial state. The requested command is:

```text
cargo bench --locked --bench state_machine_benchmarks --all-features -- --noplot
```

## Legacy unisolated invocations

Four legacy invocations exited 0 and reported all three selected benchmark names. They used Cargo's default worktree `target/criterion` directory, without `CARGO_TARGET_DIR`; concurrent host build activity was later observed during this measurement window. They are retained as historical diagnostics only and do not count as clean samples.

Legacy command:

```text
cargo bench --locked --bench state_machine_benchmarks --all-features -- --noplot 'standard_state_machine/(self_transition|task_lifecycle|business_rejection)'
```

Legacy run 1 raw output was saved, but its estimate artifacts were overwritten before capture. A hash comparison showed that every one of the 12 run 2 JSON files was byte-identical to the saved pre-existing Criterion base. The copied run-2 directory has therefore been renamed `superseded-run-2-duplicate-of-preexisting-cache/` and is excluded. The run-3 and run-4 estimate copies came from the then-current target's `new/` directories, but the shared target was not isolated and the host was not confirmed idle; these data are excluded too. Criterion's `change` lines refer to the old shared cache and are not analyzed.

## Isolated-target diagnostic invocations

Three more commands used `CARGO_TARGET_DIR=/tmp/qubit-rsm003-isolated-target-20261103`. I inspected the actual output tree at `/tmp/qubit-rsm003-isolated-target-20261103/criterion/standard_state_machine/{scenario}/new/` after each invocation and copied each `new` directory immediately. SHA-256 hashes of the copied estimate JSON match the source target files. However, concurrent Cargo/rustc activity on the host was observed during this measurement window, so all three are classified as contaminated diagnostics and excluded. Their outputs and artifacts are under `doc/performance-evidence/rsm003/isolated/`.

Exact isolated command (repeated three times):

```text
CARGO_TARGET_DIR=/tmp/qubit-rsm003-isolated-target-20261103 cargo bench --locked --bench state_machine_benchmarks --all-features -- --noplot 'standard_state_machine/(self_transition|task_lifecycle|business_rejection)'
```

## Clean isolated-target runs

After an explicitly authorized quiet window, three consecutive runs used the isolated target. Immediately before each run I checked `uptime` and `ps -C cargo -C rustc -C cargo-llvm-cov -o pid,comm,args --no-headers`; each check showed no matching processes. After each run the same process check again showed no matching processes. Each run's output reported all three selected scenarios, exited 0, and its actual target `new/` directory was copied before the next run. SHA-256 hashes for each copied estimate file match its source in the isolated target.

Command used for runs 5, 6, and 7 (with respective output captured to `isolated/run-5.txt`, `run-6.txt`, and `run-7.txt`):

```text
CARGO_TARGET_DIR=/tmp/qubit-rsm003-isolated-target-20261103 cargo bench --locked --bench state_machine_benchmarks --all-features -- --noplot 'standard_state_machine/(self_transition|task_lifecycle|business_rejection)'
```

| Run | Load before → after (1/5/15 min) | Self transition | Business rejection | Task lifecycle |
| --- | --- | --- | --- | --- |
| 5 | 0.66/0.32/0.16 → 0.73/0.36/0.17 | 814.39 ns [810.42, 820.41] | 103.95 ns [103.59, 104.41] | 1.6787 µs [1.6604, 1.7035] |
| 6 | 0.67/0.35/0.17 → 0.66/0.37/0.18 | 822.30 ns [810.86, 841.44] | 103.51 ns [103.46, 103.56] | 1.6609 µs [1.6569, 1.6639] |
| 7 | 0.61/0.36/0.18 → 0.54/0.36/0.18 | 814.22 ns [812.26, 815.65] | 107.22 ns [107.07, 107.39] | 1.6310 µs [1.6278, 1.6351] |

These are current-path observations only. The semantic gate still rules out a proven equivalent optimization candidate, so they are not an A/B comparison and do not establish an improvement. Earlier legacy runs 1–4 and isolated diagnostics 1–3 remain excluded as described above.

Reported timing intervals from the isolated diagnostics, retained for traceability only, were:

| Contaminated run | Self transition | Business rejection | Task lifecycle |
| --- | --- | --- | --- |
| Isolated 1 | 1.0635 µs [1.0067, 1.1261] | 105.10 ns [104.05, 106.60] | 1.6853 µs [1.6500, 1.7426] |
| Isolated 2 | 1.5601 µs [1.4677, 1.6761] | 246.18 ns [203.85, 291.15] | 7.0712 µs [5.9279, 8.1285] |
| Isolated 3 | 1.3679 µs [1.2540, 1.5268] | 170.59 ns [159.23, 185.25] | 4.9752 µs [3.7642, 6.2839] |

The large cross-run spread in the excluded diagnostics confirms why they were not used. Runs 5–7 were collected in the authorized quiet window and are the retained clean current-path observations.

## Outcome and limitations

The semantic gate fails, so production code remains unchanged. T3's structural review and three clean current-path measurements are complete; no equivalent candidate qualified for A/B testing, and no optimization benefit is claimed. No Cargo test command was run for T3, and no test claim is made here.
