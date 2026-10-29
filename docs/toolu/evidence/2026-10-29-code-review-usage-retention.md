# Code-review usage retention: release measurement

**Issue:** Falconiere/toolu#750  
**Platform:** macOS, aarch64  
**Baseline source:** `6047c50edd68860b775aa1e87d52d1303c70eb93` `crates/code-review/src/pipeline/state.rs` (identical to `origin/main` at measurement time)  
**Changed source:** this branch's `crates/code-review/src/pipeline/state.rs`

The same `cargo test --release -p toolu-code-review usage_retention_release_measurement -- --nocapture` workload ran against each source version. The test makes 60 real local `log_usage` calls per sample into a fresh temporary project state root. Each row is about 100 KiB of JSONL with a distinct diff hash. It records retained files and bytes after all writes, elapsed write time, and process file descriptors from `/dev/fd`. There are three samples per version; no network calls or other workloads are part of this test. The baseline source was swapped into this worktree only for its release build and restored by a shell trap immediately afterward.

| Source | Sample | Retained bytes | Files | FDs before/after | Elapsed ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline | 1 | 6,007,190 | 1 | 4 / 4 | 26 |
| Baseline | 2 | 6,007,190 | 1 | 4 / 4 | 24 |
| Baseline | 3 | 6,007,190 | 1 | 4 / 4 | 24 |
| Changed | 1 | 5,006,000 | 5 | 4 / 4 | 5,732 |
| Changed | 2 | 5,006,000 | 5 | 4 / 4 | 714 |
| Changed | 3 | 5,006,000 | 5 | 4 / 4 | 196 |

The baseline kept all 60 rows in one growing file. The changed build kept the latest 50 complete rows in one active file and four archives, each below 1 MiB. File descriptor counts returned to the starting value. The changed timings varied widely while shared epic jobs and machine load were active, so these samples establish the retained-storage bound, not a latency improvement. An exploratory build with `sync_data` took 502–781 ms for the same 60 writes; that additional flush was removed because the original usage writer did not promise crash durability and the issue requires preserving its append-failure behavior.

Thread count was not sampled because this append path creates no threads. The concurrent regression uses eight real writer threads to verify serialization and exact-once successful rows at rotation.
