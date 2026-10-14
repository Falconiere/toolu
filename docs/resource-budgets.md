# Resource budgets

The Rust rebuild (epic #402) exists to cut what every hook spawn costs. This page holds every resource budget other issues point at, where each number came from, and how it is measured. Hook budgets are machine data in `benchmarks/hook-budgets.json`, and CI gates them; so is the `toolu --version` startup budget, in `benchmarks/startup-budgets.json`. The other rows are the numbers their owning issues measure their real implementation against.

## Budgets

| Budget | Number | Measured from | How it is measured | Owner |
|---|---|---|---|---|
| `toolu hook agent-tier` | ≤ 8 MiB max RSS, ≤ 4 ms CPU (p50) | Linux CI: 4.7 MiB, 2.7 ms | `cargo xtask bench-hooks --assert`, per hook spawn | #425 |
| `toolu hook check-binary` | ≤ 7 MiB max RSS, ≤ 3 ms CPU (p50) | Linux CI: 4.4 MiB, 2.0 ms | as above | #425 |
| `toolu hook mcp-tools` | ≤ 7 MiB max RSS, ≤ 2 ms CPU (p50) | Linux CI: 4.4 MiB, 1.3 ms | as above | #425 |
| `toolu hook pre-tools` | ≤ 8 MiB max RSS, ≤ 5 ms CPU (p50) | Linux CI: 6.1 MiB, 1.7 ms | `cargo xtask bench-hooks --assert`, per hook spawn, `sh` launcher included | #418–#422 |
| `toolu hook post-tools` | ≤ 6 MiB max RSS, ≤ 10 ms CPU (p50), excluding external linters it runs on purpose | prototype: 3.6 MiB, 2.6 ms | as above | #423 |
| `toolu hook pre-compact` | ≤ 6 MiB max RSS, ≤ 4 ms CPU (p50) | native: 3.8 MiB, 3.0 ms | `cargo xtask bench-hooks`, release `toolu`, `sh` launcher included | #424 |
| `toolu hook session-start` | ≤ 8 MiB max RSS, ≤ 43 ms CPU (p50) | native: 4.8 MiB, 33.8 ms | as above | #424 |
| `toolu hook user-prompt-submit` | ≤ 9 MiB max RSS, ≤ 16 ms CPU (p50) | native: 5.5 MiB, 12.6 ms | as above | #424 |
| Binary size | ≤ 4 MiB | prototype: 2.75 MiB | size of the stripped release `toolu` (fat LTO, one codegen unit), Linux x86_64 | #417 |
| `toolu --version` startup | ≤ 4 ms wall (p50) | prototype: 2.3 ms | `cargo xtask check-startup --bin <toolu>`: wall time from spawn to reap of one `toolu --version`, 30 runs after 3 warm-up, nearest-rank p50; the Linux `rust` CI job gates the release binary | #442 |
| One statusline render | ≤ 4 ms wall, ≤ 5 MiB max RSS (p50) | prototype: 2.6 ms, 3.9 MiB | `cargo xtask bench-statusline --assert`: 30 warm-cache release-binary renders after 3 warm-ups through `cargo xtask measure`; render reads cached Git counts; Linux `rust` CI gates it | #431 |
| Idle engine RSS | ≤ 5 MiB | prototype: 3.9 MiB | resident set (`VmRSS`) of the idle resident engine 2 s after start | #434 |
| Status server under 50 clients | ≤ 7 ms latency (p90), ≤ 13 MiB max RSS | prototype: 4.4 ms, 9.7 MiB | 50 concurrent keep-alive clients × 40 loopback requests; client-side latency, server `VmHWM` | #449 |
| Gate duration | ≤ 60 s | 40 s (slowest of three CI runs) | the `cargo xtask gate` step of the `rust (ubuntu-latest)` job, warm cache | #455 |

Linux CI is the gating platform. macOS is never gated: the hook bench measures and reports there, and the startup check runs on Linux only.

The status-server check is `cargo test -p toolu-epic-orchestrator --test status_budget` after a release build of `toolu`. It keeps eight SSE connections open while 50 clients each send 40 sequential keep-alive requests to `/api/v1/overview`; the test reads the server process's `/proc/<pid>/status` `VmHWM` and calculates nearest-rank client p90. A Rust 1.99 aarch64 Linux container run on 2026-10-11 measured 0.199 ms p90 across 2,000 requests and 5,984 KiB high-water RSS. CI runners can produce different values, so the test enforces the published limits there too.

## Where the numbers come from

The original pre/post hook rows come from `benchmarks/results/hook-prototype-2026-10-05.json`. The prototype was a single `toolu` binary with the planned shape:

- a clap tree for every namespace;
- ureq and rustls linked;
- brush-parser;
- serde_json payloads;
- reads of the config, the registry, the gate file and `.git`;
- a `toolu hook` fast path.

It was built out of tree, because Rust outside a workspace member fails `cargo xtask check-reach`. The result file records its exact shape and dependency versions.

The three hook budgets introduced with #425 use the measured Linux CI run in `benchmarks/results/hook-resources-rust-linux-x64-ci-2026-10-12.json`, with RSS at p50 × 1.5 and CPU at p50 × 1.25, each rounded up. Existing budgets are unchanged.

Pre-tools was recalibrated from `benchmarks/results/hook-resources-rust-linux-x64-ci-2026-10-13.json` after the merged binary outgrew the prototype (6.1 MiB p50 against a 6 MiB budget): RSS is the rule applied to the new run — min(epic start 8 MiB, p50 × 1.5 rounded up) — and CPU keeps the epic 5 ms.

Rule:

- **RSS and size:** the prototype has the binary's full shape, so hook RSS tightens to its p50 × 1.5, rounded up to a whole MiB, when that is below the epic's starting budget (8 MiB pre-tools, 10 MiB post-tools). Every other RSS or size budget is its measurement × 1.25, rounded up.
- **CPU and time:**
  - The prototype pays only the fixed cost: payload, config, registry, gate and `.git` reads, plus the shell parse. It runs none of the gate or rule work, so the hook CPU budgets keep the epic's 5 ms and 10 ms, which the prototype meets with room for that work. Had it exceeded them, they would have been raised to its p50 × 1.25.
  - Every other time budget is its measurement × 1.5, rounded up to a whole ms or s.

A budget changes only together with a new measurement committed under `benchmarks/results/`, in a PR of its own. The session lifecycle rows (#424) — `toolu/session-start`, `toolu/user-prompt-submit` and `toolu/pre-compact` — come from `benchmarks/results/hook-lifecycle-2026-10-07.json`: RSS is that run's p50 × 1.5, rounded up to a whole MiB, and CPU is its p50 × 1.25, rounded up to a whole millisecond.

## Requirements the budgets rest on

- **`toolu hook` takes a fast path.** `toolu hook <name>`, `toolu <plugin> hook <name>` and `toolu --hook-protocol` are recognised before the clap command tree is built (`crates/cli/src/fast.rs`). Building the tree first cost the prototype's pre-tools 0.9 ms CPU and 0.4 MiB at p50 (3.4 ms against 2.5 ms), a fifth of its CPU budget. Owned by #442 and #418.
- **Hooks are measured in the shape their budgets were set in.** Each spawn runs `/bin/sh -c 'exec "$0" "$@"' toolu [<plugin>] hook <name> --event <Event> --plugin-root <dir>` with the release binary: the `sh` a host starts, then the hook. The launcher's binary resolution, whose `--hook-protocol` probe adds about 1–2 ms CPU on the Linux runner, is left out. Measuring the full launcher needs re-measured budgets, which change only in a PR of their own.

## How hooks are measured

`cargo xtask bench-hooks [--bin <toolu>] [--runs N] [--only <plugin/hook>]... [--out FILE] [--assert]` (`crates/xtask/src/bench_hooks*.rs`, #440) does the following:

1. Discovers every entry that `plugins/*/hooks/hooks.json` launches, one per `<plugin>/<hook name>`, and requires `benchmarks/cases/hooks/payloads.json` to cover exactly those.
2. Runs each one with its fixed payload. The sandbox is a git project with toolu and the four registry plugins installed, and their `register` hooks run once. For `toolu/post-tools` only, a second git sandbox has toolu alone installed: its budget excludes other plugins' external quality linters.
3. Spawns each entry twice unmeasured, then `--runs` times (default 10), one at a time, each in a fresh `xtask measure` process.
4. Prints p50 and p90 of max RSS, CPU (user + sys, children included) and wall per entry, and writes the `toolu.hook-resources/v1` document to `--out`.

`--assert` fails, naming the entry, the metric, the measured value and the budget, when an entry of `benchmarks/hook-budgets.json` exceeds its p50 budget, or when the measured phase takes over 60 s. An entry without a budget is measured and reported, not gated. The `hook-bench` CI job runs it on `ubuntu-latest` with `--assert` and on `macos-latest` without, and uploads the JSON result.

Each spawn goes through `cargo xtask measure --out FILE -- COMMAND…`, run by a release `xtask`: a debug `cargo xtask` builds `xtask --release` into its own target directory first, and `XTASK_MEASURER` names another measurer (the bench's own tests use the test build). That task spawns the command once with inherited stdio, waits, and reads `getrusage(RUSAGE_CHILDREN)`. With one child, that is the child's whole process tree. `ru_maxrss` is KiB on Linux and bytes on macOS, and the task reports bytes.

On Linux, `exec` folds the spawner's RSS high-water mark into the child's, so no measurement reads below the measurer's own RSS. The bench reports that floor (`/usr/bin/true` measured the same way): about 3 MiB on the CI runner, 1.3 MiB on macOS. A reading above it is exact. A reading at it means "at most the floor". The same is why every sample gets its own measurer: `ru_maxrss` of `RUSAGE_CHILDREN` is a high-water mark over every child a process has reaped.

RSS is in MiB (1024² bytes).

## Bun baseline

The committed baselines show what the retired Bun bundles cost on the CI runners, next to the native results under `benchmarks/results/hook-resources-rust-*.json`. They are the source files for this table:

- `benchmarks/results/hook-resources-bun-linux-x64-ci-2026-10-05.json`
- `benchmarks/results/hook-resources-bun-darwin-arm64-ci-2026-10-05.json`

| Entry | Linux CI RSS / CPU (p50) | macOS CI RSS / CPU (p50) |
|---|---|---|
| `toolu/pre-tools` | 48.2 MiB / 91 ms | 37.2 MiB / 57 ms |
| `toolu/post-tools` | 55.6 MiB / 166 ms | 45.1 MiB / 120 ms |
