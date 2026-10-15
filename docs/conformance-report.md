# Cross-host conformance report

**Issue:** [#212](https://github.com/Falconiere/toolu/issues/212)  
**Status:** historical for the TypeScript harness. The Bun conformance runner (`bun run test:conformance`, `tools/toolu-conformance`) was removed in #440. Every test is a Rust test now: see [testing.md](testing.md). The live check of both hosts is `cargo xtask host-smoke --host claude|codex|both`.

## Platforms

| OS | Status |
|----|--------|
| macOS | The plugin runs in CI on `macos-14` (`rust-macos`) and `macos-latest` (`hook-bench`). |
| Linux | The plugin runs in CI on `ubuntu-24.04`. |
| Windows | **N/A**: not probed for this port |

## Fixture suites

The suites below were the harness's matrix. Their Rust replays are `crates/cli/tests/pre_tool_gates.rs` and `golden_gates.rs`; a suite that cannot run fails.

| Suite | What it proves |
|-------|----------------|
| `protected-files` | The native PreToolUse hook blocks a `.env` edit (`deny` or `ask`); **file bytes are unchanged on `deny`** |
| `spaces-cwd` | A project path with spaces still blocks a protected edit |

## Isolation

Test sandboxes use `TMPDIR` (recommended: `/Volumes/Projects/.tmp` locally) and set `TOOLU_CONFIG_DIR` / an isolated `HOME` so Claude/Codex host settings are not mutated.

## Final v7.2.0 Bash comparison (#279)

This is a record of the v7.2.0 measurement. The Bun side was the committed hook bundles, which #440 removed; the native hooks are measured by `cargo xtask bench-hooks` against the budgets in [resource-budgets.md](resource-budgets.md).

Each row below is the p50 of 15 sequential Bash/Bun pairs after two warm-up pairs, in milliseconds. The Bash side was extracted from tag `v7.2.0` with `git archive`; the Bun side used the committed hook bundles. macOS ran natively on an Apple M2 Max with Bun 1.4.2. Linux ran inside a Debian arm64 Colima container on the same host with Bun 1.4.2, Git and jq; both sides of each Linux pair ran in that same container. A negative delta means Bun was faster. The end-to-end rows include command launch, parsing, gate work, and fixture I/O; they are diagnostic comparisons.

| Hook fixture | macOS Bash | macOS Bun | macOS delta | Linux Bash | Linux Bun | Linux delta |
|---|---:|---:|---:|---:|---:|---:|
| Pre: protected `.env` edit | 309.0 | 95.4 | -213.5 | 93.5 | 34.2 | -59.4 |
| Pre: feature docs | 267.1 | 83.5 | -183.6 | 88.8 | 34.6 | -54.2 |
| Pre: multi-file patch | 495.1 | 60.4 | -434.7 | 159.2 | 33.9 | -125.3 |
| Pre: plain command | 250.7 | 64.1 | -186.6 | 76.1 | 34.8 | -41.3 |
| Pre: commit advice | 382.5 | 105.2 | -277.3 | 113.4 | 38.6 | -74.8 |
| Pre: unreviewed push | 909.7 | 339.3 | -570.4 | 217.5 | 49.0 | -168.5 |
| MCP: listed server | 49.9 | 48.1 | -1.8 | 17.7 | 16.1 | -1.6 |
| MCP: unlisted server | 33.8 | 48.9 | **+15.1** | 11.9 | 15.8 | +3.9 |
| MCP: no blocklist or config | 22.6 | 44.9 | **+22.3** | 8.2 | 11.5 | +3.3 |
| Post: failing quality command | 190.4 | 85.7 | -104.8 | 53.8 | 27.4 | -26.4 |
| Post: first passing quality command | 103.6 | 67.1 | -36.5 | 30.7 | 25.4 | -5.4 |
| Post: plain command | 85.3 | 61.7 | -23.6 | 27.5 | 25.2 | -2.3 |
| Post: push waiver promotion | 172.1 | 123.5 | -48.6 | 38.3 | 28.5 | -9.9 |
| Post: multi-path registry patch | 362.3 | 83.8 | -278.5 | 115.9 | 33.1 | -82.9 |

The owner's 5 ms **incremental cold-start** budget applied to the readable unminified shell-analysis bundle probe (`bun run bench:shell --assert`, removed in #440), not to these full hook rows. On the macOS run, the 40-pair probe measured empty bundle p50 **28.92 ms**, combined shell-analysis bundle p50 **32.71 ms**, delta **+3.78 ms** and passed. The owner-provided Linux CI measurements were **+5.26 ms on AMD** and **+6.84 ms on Intel**; both exceeded 5 ms and stayed report-only. These Linux CI numbers are from the owner's earlier runner evidence, not from the Linux arm64 container above. The full hook table preserves the macOS MCP fast-path overages; no claim that every end-to-end row is within 5 ms is made.
