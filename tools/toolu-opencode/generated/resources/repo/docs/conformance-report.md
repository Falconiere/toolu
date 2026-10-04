# Cross-host conformance report

**Issue:** [#212](https://github.com/Falconiere/toolu/issues/212)\
**Depth:** fixture suites (hermetic temp projects, native dispatcher, Bun bootstrap), plus required OpenCode acceptance on the real host
**Runners:**
- `bun run test:conformance` → `tools/toolu-conformance/src/cli/run.ts`
- `bun run test:opencode` → `tooling/src/opencode-acceptance.ts`

## Pins

| Component | Pin |
|-----------|-----|
| Bun | `1.4.x` (workspace engines `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`) |
| OpenCode CLI | `opencode-ai@1.18.34` (`tools/toolu-opencode/contract/pin.json`; see [opencode-host-contract.md](opencode-host-contract.md)) |
| Plugin SDK | `@opencode-ai/plugin@1.18.34`, which the host provisions at its own version |

## Platforms

| OS | Status |
|----|--------|
| macOS | Supported. OpenCode acceptance runs in CI on `macos-latest` (darwin-arm64) |
| Linux | Supported. OpenCode acceptance runs in CI on `ubuntu-latest` (linux-x64) |
| Windows | **N/A**: not probed for this port |

## Fixture suites

No suite can skip: a suite that cannot run fails the matrix.

| Suite | What it proves |
|-------|----------------|
| `protected-files` | The native PreToolUse bundle blocks a `.env` edit (`deny` or `ask`); **file bytes are unchanged on `deny`** |
| `bootstrap-readiness` | A Bun registration bundle that exits 0 without registry artifacts is `NotReady` |
| `surface-drift` | `bun run check:opencode-surface` is clean |
| `spaces-cwd` | A project path with spaces still blocks a protected edit |

OpenCode is not a conformance suite. The old `opencode --version` probe and the handwritten `permission.evaluate` event were removed in [#362](https://github.com/Falconiere/toolu/issues/362). The real host replaces them, below.

## OpenCode acceptance (required)

`bun run test:opencode` runs the pinned `opencode-ai` binary in isolated profiles (temporary `HOME`, `XDG_*` and git project) against a scripted loopback model. CI runs it on Linux and macOS (`opencode (ubuntu-latest)` and `opencode (macos-latest)`). The required `typescript` status fails unless both acceptance runs and the `bun run test` job pass.

**What it runs.**

- The 29 host contract probes, compared with `contract/probe-results.json`.
- The generated surface and the names the host discovers.
- Every `smoke:opencode-*` scenario.
- The seven `*.live.test.ts` files, through a JUnit report, so a skipped host test fails.
- Three concurrent runs over two projects.
- A startup and per-tool budget.

Every one of the 16 catalog plugins must have a passing dedicated actual-host check.

**Preflight.** A missing host, `git`, `npm`, `tar`, `ast-grep`, `agent-browser` or its Chromium fails the run before any session, and names what is missing. Nothing is reported as skipped.

**Regression controls.** Each run stages copies of `@toolu/opencode`. A control is detected only when its check passes against the unbroken copy and then fails once one edit breaks it. A stage that fails on its own, or an edit that no longer applies, counts as a missed control:

| Control | Regression | Check that must fail |
|---|---|---|
| `control.v2-entry` | The entry is the V2 `Plugin.define` / `permission.hook("evaluate")` shape | `entry.local-shim` |
| `control.invalid-skill-name` | A generated skill id has a double hyphen | `surface.discovered-names` |
| `control.missing-context` | `experimental.chat.system.transform` is not wired | `live.context-delivery` |
| `control.missing-post-tool` | `tool.execute.after` is not wired | `posttool.edit` |

**Evidence.** The report keeps three categories apart:

- **Execution.** `actual-host` means the pinned binary ran; `in-process` means the hooks were called directly.
- **Model.** Always `scripted-loopback`: a fixture that only scripts replies.
- **Service.**
  - `none`.
  - `fixture`: a loopback HTTP(S) server or bare git remote stands in for exa, context7, Jev, Jira or GitHub.
  - `live`: real external services are reported under `external` and never decide acceptance. context7.com is called without a key. api.typesafe.ai is called only when `TYPESAFE_API_KEY` is set and is otherwise `not-configured`.

The run removes service credentials (`CONTEXT7_API_KEY`, `EXA_API_KEY`, `TYPESAFE_API_KEY`, `JIRA_*`) from its own environment, so no fixture check can reach a live service.

**Budgets.** The same six-call session runs twice with no plugin (a no-op local plugin) and twice with toolu selecting every plugin. The startup overhead (spawn to first model request) and the median per-tool overhead are toolu's fastest run minus the reference's fastest. They must stay within `contract/acceptance-budgets.json`. On darwin-arm64, two runs measured +444 and +539 ms startup and +45 and +35 ms per tool call. The ceilings leave a wide margin for shared CI runners.

**Report.** The JSON report holds:

- the CLI version, the provisioned SDK version, the platform, Bun and every tool version;
- each check with its plugins, evidence, duration and observations;
- the controls, the budget and the coverage.

The CI jobs upload it as `opencode-acceptance-<os>`, and the Markdown summary goes to the job summary.

```bash
bun run test:opencode                              # complete run; exits 0 only on acceptance
bun run test:opencode --only entry.npm-root pretool.mcp control.missing-post-tool   # local narrowing, never acceptance
```

For #276, the old fixture matrix passed with a temporary `@opencode/cli@2.0.12` binary on a PATH containing only Bun and git. That V2 lane no longer exists.

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
