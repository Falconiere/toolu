# Resource budgets

The Rust rebuild (epic #402) exists to cut what every hook spawn costs. This page holds every resource budget other issues point at, where each number came from, and how it is measured. Hook budgets are machine data in `benchmarks/hook-budgets.json`, and CI gates them. The other rows are the numbers their owning issues measure their real implementation against.

## Budgets

| Budget | Number | Measured from | How it is measured | Owner |
|---|---|---|---|---|
| `toolu hook pre-tools` | ≤ 6 MiB max RSS, ≤ 5 ms CPU (p50) | prototype: 3.6 MiB, 2.5 ms | `bun run bench:hooks --assert`, per hook spawn, `sh` launcher included | #418–#422 |
| `toolu hook post-tools` | ≤ 6 MiB max RSS, ≤ 10 ms CPU (p50), excluding external linters it runs on purpose | prototype: 3.6 MiB, 2.6 ms | as above | #423 |
| Binary size | ≤ 4 MiB | prototype: 2.75 MiB | size of the stripped release `toolu` (fat LTO, one codegen unit), Linux x86_64 | #417 |
| `toolu --version` startup | ≤ 4 ms wall (p50) | prototype: 2.3 ms | wall time of one spawn through `cargo xtask measure`, 30 runs after 3 warm-up | #442 |
| One statusline render | ≤ 4 ms wall, ≤ 5 MiB max RSS (p50) | prototype: 2.6 ms, 3.9 MiB | one `toolu statusline render` with a session payload through `cargo xtask measure` | #431 |
| Idle engine RSS | ≤ 5 MiB | prototype: 3.9 MiB | resident set (`VmRSS`) of the idle resident engine 2 s after start | #434 |
| Status server under 50 clients | ≤ 7 ms latency (p90), ≤ 13 MiB max RSS | prototype: 4.4 ms, 9.7 MiB | 50 concurrent keep-alive clients × 40 loopback requests; client-side latency, server `VmHWM` | #449 |
| Gate duration | ≤ 60 s | 40 s (slowest of three CI runs) | the `cargo xtask gate` step of the `rust (ubuntu-latest)` job, warm cache | #455 |

Linux CI is the gating platform. macOS is measured and reported, never gated.

## Where the numbers come from

The hook rows come from `benchmarks/results/hook-prototype-2026-10-05.json`. The prototype was a single `toolu` binary with the planned shape:

- a clap tree for every namespace;
- ureq and rustls linked;
- brush-parser;
- serde_json payloads;
- reads of the config, the registry, the gate file and `.git`;
- a `toolu hook` fast path.

It was built out of tree, because Rust outside a workspace member fails `cargo xtask check-reach`. The result file records its exact shape and dependency versions.

Rule:

- **RSS and size:** the prototype has the binary's full shape, so hook RSS tightens to its p50 × 1.5, rounded up to a whole MiB, when that is below the epic's starting budget (8 MiB pre-tools, 10 MiB post-tools). Every other RSS or size budget is its measurement × 1.25, rounded up.
- **CPU and time:**
  - The prototype pays only the fixed cost: payload, config, registry, gate and `.git` reads, plus the shell parse. It runs none of the gate or rule work, so the hook CPU budgets keep the epic's 5 ms and 10 ms, which the prototype meets with room for that work. Had it exceeded them, they would have been raised to its p50 × 1.25.
  - Every other time budget is its measurement × 1.5, rounded up to a whole ms or s.

A budget changes only together with a new measurement committed under `benchmarks/results/`, in a PR of its own.

## Requirements the budgets rest on

- **`toolu hook` takes a fast path.** It dispatches on `argv[1] == "hook"` before building the full clap command tree. Building the tree first cost the prototype's pre-tools 0.9 ms CPU and 0.4 MiB at p50 (3.4 ms against 2.5 ms), a fifth of its CPU budget. Owned by #442 and #418.
- **Hooks are measured the way hosts pay for them.** Each spawn includes the `sh` launcher. Until the native launcher (#412) exists, the bench runs a ported entry as `/bin/sh -c 'exec "$0" "$@"' toolu hook <entry>`, the same shape as the Bun launcher.

## How hooks are measured

`bun run bench:hooks` (`tooling/src/benchmarks/hook-resources.ts`) does the following:

1. Discovers every entry that `plugins/*/hooks/hooks.json` launches.
2. Runs each one with its fixed payload from `benchmarks/cases/hooks/payloads.json`. The sandbox is a git project with toolu and the four registry plugins installed, and their `register` hooks run once. That is the fixed registry fixture.
3. Spawns each entry `--warmup` times (default 2), then `--runs` times (default 10), one at a time.
4. Reports p50 and p90 of max RSS, CPU (user + sys, children included) and wall per entry.

`--assert` measures the entries listed in `fixtures/rust-ported.json` as Rust and fails, naming the entry, the metric, the measured value and the budget, when one exceeds its p50 budget. It also fails when a ported entry has no budget, or when the measured phase takes over 60 s. The `hook-bench` CI job runs it on `ubuntu-latest` with `--assert` and on `macos-latest` without, and uploads the JSON result.

Each spawn goes through `cargo xtask measure --out FILE -- COMMAND…`. That task spawns the command once with inherited stdio, waits, and reads `getrusage(RUSAGE_CHILDREN)`. With one child, that is the child's whole process tree. `ru_maxrss` is KiB on Linux and bytes on macOS, and the task reports bytes.

On Linux, `exec` folds the spawner's RSS high-water mark into the child's, so no measurement reads below the measurer's own RSS. The bench reports that floor (`/usr/bin/true` measured the same way): about 3 MiB on the CI runner, 1.3 MiB on macOS. A reading above it is exact. A reading at it means "at most the floor". This is also why Bun cannot measure hooks itself: its children inherit its own 14–22 MB.

RSS is in MiB (1024² bytes).

## Bun baseline

The committed baselines show what the Bun bundles cost on the CI runners. They are the source files for this table:

- `benchmarks/results/hook-resources-bun-linux-x64-ci-2026-10-05.json`
- `benchmarks/results/hook-resources-bun-darwin-arm64-ci-2026-10-05.json`

| Entry | Linux CI RSS / CPU (p50) | macOS CI RSS / CPU (p50) |
|---|---|---|
| `toolu/pre-tools` | 48.2 MiB / 91 ms | 37.2 MiB / 57 ms |
| `toolu/post-tools` | 55.6 MiB / 166 ms | 45.1 MiB / 120 ms |
