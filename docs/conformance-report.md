# Cross-host conformance report

**Issue:** [#212](https://github.com/Falconiere/toolu/issues/212)  
**Depth:** fixture-suite (hermetic temp projects; native dispatcher and Bun bootstrap)
**Runner:** `bun run test:conformance` → `tools/toolu-conformance/src/cli/run.ts`

## Pins (V2 adapter suites)

These suites exercise the shipped V2 adapter. The documented-host contract (`opencode-ai@1.18.34`, `@opencode-ai/plugin@1.18.34`) and its live probes are in [opencode-host-contract.md](opencode-host-contract.md). OP-28 ([#362](https://github.com/Falconiere/toolu/issues/362)) replaces this lane with real-host acceptance.

| Component | Pin |
|-----------|-----|
| Bun | `1.4.x` (workspace engines `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`) |
| OpenCode CLI | `v2.0.12` (`$OPENCODE_BIN` or `command -v opencode`) |
| Plugin SDK | `@opencode/plugin@2.0.12` |

## Platforms

| OS | Status |
|----|--------|
| macOS | Supported (Bun 1.4.x and git for OpenCode) |
| Linux | Supported (same OpenCode prerequisites) |
| Windows | **N/A** — not probed for this port |

## Fixture suites

| Suite | What it proves |
|-------|----------------|
| `protected-files` | Native PreToolUse bundle blocks `.env` edit (`deny` or `ask`); **file bytes unchanged on `deny`** |
| `bootstrap-readiness` | A Bun registration bundle exits 0 without registry artifacts → `NotReady` |
| `permission-evaluate` | `permission.evaluate` handler + block mode → `deny` on protected `.env` |
| `surface-drift` | `bun run check:opencode-surface` clean |
| `spaces-cwd` | Project path with spaces still blocks protected edit |
| `live-opencode` | Optional live CLI probe (see below) |

## Live OpenCode lane (optional)

CI does **not** require a live OpenCode install. The `live-opencode` suite records **skip** and the matrix still exits 0.

To run locally:

```bash
export TOOLU_LIVE_OPENCODE=1
# optional: export OPENCODE_BIN=/path/to/opencode
bun run test:conformance
```

When `TOOLU_LIVE_OPENCODE=1`, the runner executes `opencode --version` (or `$OPENCODE_BIN --version`) and fails the matrix if that command is missing or non-zero.

For #276, the full fixture matrix passed with a temporary `@opencode/cli@2.0.12` binary, `TOOLU_LIVE_OPENCODE=1`, and a PATH containing only Bun and git. The live lane verifies the pinned CLI version; the separate OpenCode adapter test stages the npm catalog, bootstraps `ast-grep` into an isolated config root, and checks its in-process advisory with the same bash-free PATH.

## Isolation

Conformance fixtures use `TMPDIR` (recommended: `/Volumes/Projects/.tmp` locally) and bootstrap paths set `TOOLU_CONFIG_DIR` / isolated `HOME` so Claude/Codex host settings are not mutated.

## Final Bun-only clean-install smoke (#279)

On 2026-10-01, `bun run tooling/src/clean-install-smoke.ts` passed with temporary `HOME`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `TOOLU_CONFIG_DIR`, `XDG_CONFIG_HOME`, and `OPENCODE_CONFIG_DIR`. No command in this smoke targeted the user's real host configuration. Claude Code and Codex each installed `toolu` from the checkout's local marketplace into their temporary config root; the installed bundle denied an edit of a temporary project's `.env` and left its bytes unchanged. OpenCode v2.0.12 installed a staged local Git package into its temporary config, then the staged package bootstrapped its native adapter and denied the same protected edit. The OpenCode permission call exercised the adapter directly after CLI installation; it did not launch an interactive OpenCode session.

## Final v7.2.0 Bash comparison (#279)

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

The owner's 5 ms **incremental cold-start** budget applies to the readable unminified shell-analysis bundle probe (`bun run bench:shell --assert`), not to these full hook rows. It is a hard assertion on macOS arm64 or with `TOOLU_LATENCY_ENFORCE=1`; Linux reports the measured delta without failing that budget. On this macOS run, the 40-pair probe measured empty bundle p50 **28.92 ms**, combined shell-analysis bundle p50 **32.71 ms**, delta **+3.78 ms** and passed. The owner-provided Linux CI measurements were **+5.26 ms on AMD** and **+6.84 ms on Intel**; both exceed 5 ms and remain report-only. These Linux CI numbers are from the owner's earlier runner evidence, not from the Linux arm64 container above. The full hook table preserves the macOS MCP fast-path overages; no claim that every end-to-end row is within 5 ms is made.
