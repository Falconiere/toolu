# OpenCode CI acceptance — Design

**Date:** 2026-10-04   **Status:** Approved   **Author:** Claude Code   **Topic:** OP-28 (#362): replace version-only probes with real OpenCode acceptance that CI requires

## Problem

Nothing in CI runs OpenCode. Two lanes only look like acceptance:

1. **Version-only probe.** The conformance suite `live-opencode` (`tools/toolu-conformance/src/cli/suites/live-opencode.ts`) runs `opencode --version`, and only when `TOOLU_LIVE_OPENCODE=1`. Otherwise it records `skip`, and the matrix still passes.
2. **Handwritten permission event.** The conformance suite `permission-evaluate` calls the retained V2 `createPermissionEvaluateHandler` with an event it builds itself. The host never runs.

The real evidence exists but runs only by hand:

- `probe:opencode-host` (29 contract probes) and `probe:opencode-surface`;
- ten `smoke:opencode-*` scripts (about 70 scenarios);
- seven `*.live.test.ts` files that `test.skipIf(TOOLU_LIVE_OPENCODE !== "1")` skips in `bun run test`.

Together they reach all 16 catalog plugins on the pinned `opencode-ai@1.18.34`, but on Linux only (the contract doc says macOS belongs to this issue). Nothing proves that the scenarios would notice the regressions the epic was opened for. Nothing records host or SDK versions, timings or which evidence came from a fixture.

Measured on darwin-arm64 at `24e65189`, the whole set took about 8 minutes. Every check passed except `permissions.other-after`. That check assumes the host runs `.opencode/plugins/*` hooks in file-creation order, which holds on ext4 but not on APFS. The pinned host instead orders config-listed plugins before directory plugins, deterministically, on both platforms (verified with a scratch probe). So the order is a harness flaw, not a product one, and it is exactly the kind of thing macOS acceptance must catch.

## Non-Goals

1. Rewriting existing scenarios. The harness changes are limited to:
   - the load-order fix;
   - a package-directory override for the regression controls;
   - a request timestamp in the scripted provider;
   - moving the surface probe's logic into an exported function, behind its unchanged script.
2. Gating on real external services (context7.com, api.typesafe.ai). They are reported as live availability and never decide acceptance.
3. Windows. It stays N/A.
4. Changing repository settings. The required status context `typescript` keeps its name.
5. Changing production plugin behavior or adding timing logs to it.
6. Deleting the retained V2 adapter (`createPermissionEvaluateHandler`, `permission-map`). `tooling/src/clean-install-smoke.ts` still drives it, and its removal is not needed here.
7. Rewriting the install and migration docs, which belong to OP-29 (#363). Only the facts this change alters are updated.

## Architecture

### 1. One acceptance command

`bun run test:opencode` runs `tooling/src/opencode-acceptance.ts`, with helpers under `tooling/src/opencode-acceptance/`. It runs in-process, serially, against one resolved pinned host and one run cache:

1. **Preflight** (`preflight.ts`).
   - Resolve the host with `resolveHostBinary`. A missing or failed install, or a version mismatch, is a failure.
   - Check `git`, `npm`, `tar`, `ast-grep` and `agent-browser` on PATH, and that `agent-browser doctor --json` reports Chromium installed. Record each version.
   - Any missing tool fails the run before a scenario starts, with one line per missing tool. Nothing skips.
2. **Warm-up.** One probe session, as in `probe:opencode-host`, records the SDK version the host provisioned.
3. **Checks**, from the registry in `checks.ts`, in this order:
   1. contract probes, compared with the committed `probe-results.json` through `resultDrift`;
   2. the generated surface;
   3. entry, startup, paths, install, browser, exa, babysit, CLI, status and package;
   4. pre-tool, permissions, post-tool, the three language-quality families and ast-grep;
   5. the live test files;
   6. the new checks `surface.discovered-names` and `concurrent.sessions`;
   7. `budget.overhead`;
   8. the four regression controls.

   Each check is timed. A thrown `ContractError` or any other error fails that check, with its message, and the run continues.
4. **Report.**
   - Write the JSON report to `--report <file>` (default `$TMPDIR/opencode-acceptance-<platform>.json`).
   - Print one line per check, then a Markdown summary. The summary is appended to `$GITHUB_STEP_SUMMARY` when that is set.
   - Exit 1 unless `pass` is true.

`--only <check-id>…` narrows a run for local work and ledger checks. A narrowed run sets `complete: false` and `pass: false` in the report, so it can never stand in for acceptance. Its exit code reflects only the selected checks and controls. The workflow test asserts that CI never passes `--only`. The existing `smoke:*` and `probe:*` scripts keep working unchanged as narrower entry points.

### 2. Check registry and evidence

Each registry entry names its checks and states what they prove:

- `plugins`: the catalog plugins it proves, or `"all"` for whole-catalog checks;
- `evidence`:
  - `execution`: `actual-host` (the pinned binary ran) or `in-process` (hooks called directly);
  - `model`: always `scripted-loopback` (the scripted provider stands in for the model);
  - `service`: `none`, `fixture` (a loopback HTTP(S) server or bare git remote stands in for an external service) or `live`.

| Family | Source | Plugins | service |
|---|---|---|---|
| `probe.*` | `SCENARIOS` + drift | none (host contract) | none |
| `surface.generated` | the logic of `opencode-surface-probe.ts`, moved into an exported function | all | none |
| `entry.*`, `paths.*`, `install.*`, `cli.*` | `ENTRY_`, `PATH_`, `INSTALL_`, `CLI_SCENARIOS` | toolu (`entry.helper-env` also context7) | none |
| `startup.*`, `package.*` | `STARTUP_`, `PACKAGE_SCENARIOS` | all | none |
| `browser.*` | `BROWSER_SCENARIOS` | agent-browser | fixture (local page) |
| `exa.*` | `EXA_SCENARIOS` | exa-search | fixture |
| `babysit.*` | `BABYSIT_SCENARIOS` | pr-babysit | fixture |
| `status.*` | `STATUS_SCENARIOS` | statusline | none |
| `pretool.*`, `permissions.*`, `posttool.*` | the smoke arrays | toolu | none |
| `tsquality.*` / `pyquality.*` / `rsquality.*` | the smoke arrays | ts-quality / python-quality / rust-quality | none |
| `ast-grep.*` | `AST_GREP_SCENARIOS` | ast-grep | none |
| `live.context-delivery` | live test file | toolu | none |
| `live.context7` | live test file | context7 | fixture |
| `live.core-workflows` | live test file | toolu, toolu-review | fixture (bare remote) |
| `live.delivery-workflows` | live test file | brainstorm, delivery-flow | fixture (bare remote) |
| `live.epic-worker` | live test file | epic-orchestrator | fixture |
| `live.jev` | live test file | jev | fixture |
| `live.jira` | live test file | jira | fixture |
| `surface.discovered-names`, `concurrent.sessions`, `budget.overhead` | new | toolu (+ all selected) | none |
| `control.*` | new | toolu | none |

**Issue scope → checks.** Every scope item of #362 maps to named checks the run requires:

| Scope item | Checks |
|---|---|
| npm, local and config loading | `entry.npm-root`, `entry.local-shim`, `entry.both-routes`, `entry.init-failure`, `package.clean-install`, `probe.load.*` |
| Discovery | `surface.generated`, `surface.discovered-names`, `install.surfaces.*` (`surfaces.npm-clean`, `.precedence`, `.skill-roots`, `.both-routes`, `.selection`, `.lifecycle`) |
| Startup | `entry.full-startup`, `entry.startup-failure`, `entry.startup-disable`, `entry.helper-env`, `entry.worktree-state` |
| Pre gates | `pretool.*` (files, patch, shell, push), `permissions.guardrail` |
| Post gates | `posttool.shell`, `posttool.edit`, `posttool.patch`, `tsquality.*`, `pyquality.*`, `rsquality.*`, `ast-grep.session` |
| Permissions | `permissions.native-deny`, `.reject`, `.judgement`, `.registry-advice`, `.other-before`, `.other-after`, `probe.permission.*` |
| MCP / task routing | `pretool.mcp`, `pretool.task`, `probe.deny.mcp`, `probe.deny.task-child`, `live.core-workflows` (task delegation) |
| Compaction | `live.context-delivery` (startup, prompt and compaction context reach the model), `probe.context.compaction` |
| Concurrent sessions | `concurrent.sessions`; child sessions in `pretool.task`, `live.epic-worker` |
| Plugin-specific | every non-`toolu` family above; `coverage` must list all 16 |
| Mandatory CI | The `opencode` matrix job and the `typescript` aggregator (§7) |
| Versions and budgets | report `host`, `tools`, `budget` (§2, `budget.overhead`) |

**Live test files** (`live-tests.ts`).

- Each file runs as `bun test --timeout 900000 --reporter=junit --reporter-outfile=<tmp> <file>`, with `TOOLU_LIVE_OPENCODE=1`.
- The JUnit `<testcase>` elements are parsed. A host test that is skipped, fails or is missing fails the check.
- The two external tests are run apart and reported under `external`, never in `checks`:
  - `one real context7.com search …` needs no key, so it always runs, with `TOOLU_LIVE_CONTEXT7=1`;
  - `one real TypeSafe judgment …` runs with `TOOLU_LIVE_JEV=1` only when `TYPESAFE_API_KEY` is set, and is otherwise `not-configured`.
- Both use `execution: in-process`, `service: live`.
- A registry test fails when a `*.live.test.ts` file exists that the registry does not list.

### 3. New live checks (`tooling/src/opencode-host/scenarios-acceptance.ts`)

- **`surface.discovered-names`.**
  - Setup: the shim route, with all 16 plugins selected. Run `opencode debug skill`.
  - Pass:
    - every skill id in the package's `generated/opencode.toolu.json` is loaded;
    - every loaded skill located under `<package>/generated/skills/` has a name that matches `^[a-z0-9]+(-[a-z0-9]+)*$`, is at most 64 characters, and equals its folder name.
  - This checks what the host actually discovered through toolu's `config` hook. The host itself accepts invalid names (probe `surface.names`).
- **`concurrent.sessions`.**
  - Setup: three `opencode run` processes start at once.
    - **P1 and P2** share project P. Each writes `.env` (refused), touches its own marker, and P1 commits `fix: one`.
    - **Q** is a second project whose quality-gate file is seeded `failing`. Q writes `.env` (refused) and commits `fix: q` (refused).
    - Both projects use `qualityGate: block`, `commitGate: off` and `pushReview: off`.
  - Pass:
    - each run logs `toolu: ready` exactly once;
    - every `.env` is byte-identical;
    - both P markers exist;
    - P's `git log` has `fix: one`;
    - Q's log lacks `fix: q`;
    - Q's gate bytes are unchanged.
- **`budget.overhead`** (`budget.ts`).
  - Setup: the same scripted session (six `bash` calls running `printf ok`) runs four times on the real host, in the order R, T, R, T:
    - **R:** a no-op local plugin, so the host's plugin-dependency wait applies to both runs;
    - **T:** toolu through the shim, with all 16 plugins selected.
  - Measures:
    - The scripted provider now stamps each recorded request with `at` (ms since epoch).
    - `startup` is the arrival of the first tool-bearing request minus spawn time.
    - `perTool` is the median gap between consecutive tool-bearing requests.
    - Each kind keeps its minimum over two runs. `startupOverheadMs = T.startup − R.startup` and `perToolOverheadMs = T.perTool − R.perTool`.
  - Pass:
    - both overheads are at most their ceilings in `tools/toolu-opencode/contract/acceptance-budgets.json`;
    - each T run logged `toolu: ready (16 plugins`;
    - each run made six tool calls.

  The ceilings are set to at least three times the largest value measured on darwin-arm64 and CI linux-x64, so they catch a pathological regression rather than runner noise. Raw samples are recorded.

### 4. Regression controls (`controls.ts`) — AC-1 made executable

Each control does three things:

1. stages a copy of `@toolu/opencode` with `stageOpencode`, linking `node_modules` from the checkout so imports resolve;
2. applies one edit through `replaceOnce(file, from, to)`, which throws unless `from` occurs exactly once;
3. runs the named check with `TOOLU_ACCEPTANCE_PACKAGE=<stage>`. `installShim` and `pretool-shared`'s `session` link that directory instead of the checkout's package; live test subprocesses inherit it.

A control is `detected` when its check does not pass. An undetected control fails the run.

| Control | Edit in the stage | Check expected to fail |
|---|---|---|
| `control.v2-entry` | `src/plugin/toolu.ts` replaced by a V2-shaped module: `export default { id: "toolu", async setup(ctx) { await ctx.permission.hook("evaluate", …) } }`, which denies `.env` under the old contract | `entry.local-shim` |
| `control.invalid-skill-name` | `generated/skills/brainstorm-brainstorm` renamed to `brainstorm--brainstorm`, in its folder, its `SKILL.md` `name:` and `generated/opencode.toolu.json` | `surface.discovered-names` |
| `control.missing-context` | `"experimental.chat.system.transform": context.system,` removed from `src/plugin/hooks.ts` | `live.context-delivery` |
| `control.missing-post-tool` | `"tool.execute.after": enforcement.after,` removed from `src/plugin/hooks.ts` | `posttool.edit` |

A hermetic test in `bun run test` applies every edit to a fresh stage. A source change that breaks an edit therefore fails the ordinary gate before CI reaches the live run.

### 5. Deterministic plugin order (`scenarios-permissions-advice.ts`)

- `permissions.other-before` puts the other plugin in `opencode.json` `plugin` (a `file://` URL outside `.opencode/plugins/`), so it runs before the directory plugins (probe and toolu).
- `permissions.other-after` removes the copied probe and shim from `.opencode/plugins/` and lists them in `opencode.json` `plugin`: the probe file and the package directory. The other plugin stays in `.opencode/plugins/`, so it runs after both.
- The `loadOrder` assertion is unchanged.

### 6. Conformance cleanup

- Delete the `live-opencode` and `permission-evaluate` suites and remove them from `CONFORMANCE_SUITES`.
- Remove `skip` from `SuiteOutcome`, so a conformance suite can no longer skip.
- `run.test.ts` asserts that every suite passes.
- Delete any helper left unused, as knip decides (`bridgeEnvOpencode` if nothing else uses it).

### 7. CI (`.github/workflows/tests.yml`)

- **`gate`.** The existing job, renamed to job id `gate` and display name `bun run test`, with its steps unchanged.
- **`opencode`.** A new matrix job, display name `opencode (${{ matrix.os }})`, over `ubuntu-latest` and `macos-latest`, with `fail-fast: false` and `timeout-minutes: 60`. Its steps:
  1. checkout and Bun 1.4.2;
  2. jq (apt, Linux only);
  3. `npm install -g @ast-grep/cli agent-browser@0.38.2`;
  4. `agent-browser install`, with `--with-deps` on Linux;
  5. `bun install --frozen-lockfile`;
  6. `actions/cache` for `~/.cache/toolu/opencode-host`, keyed on OS and the hash of `pin.json`;
  7. `bun run test:opencode --report "$RUNNER_TEMP/opencode-acceptance.json"`;
  8. upload the report as an artifact, with `if: always()`.
- **`typescript`.** A new aggregator with display name `typescript`:
  - `needs: [gate, opencode]` and `if: always()`;
  - one step fails unless `needs.gate.result` and `needs.opencode.result` are both `success`.

  The existing required context therefore fails when either job fails, is cancelled or is skipped, and the jobs still run in parallel.

## Interfaces / Schema

```ts
// tooling/src/opencode-acceptance/checks.ts
export type Evidence = {
  execution: "actual-host" | "in-process";
  model: "scripted-loopback";
  service: "none" | "fixture" | "live";
};
export type CheckOutcome = { pass: boolean; observed: Record<string, unknown> };
export type AcceptanceCheck = {
  id: string;
  family: string;
  plugins: readonly string[] | "all";
  evidence: Evidence;
  run: (ctx: AcceptanceContext) => Promise<CheckOutcome>;
};
export type AcceptanceContext = EntryContext; // { bin, cacheRoot, tarball }
export const CHECKS: readonly AcceptanceCheck[];
export function coverage(checks: readonly AcceptanceCheck[], catalog: readonly string[]): Map<string, string[]>; // plugin → dedicated check ids

// tooling/src/opencode-acceptance/live-tests.ts
export type LiveTestFile = { id: string; file: string; plugins: readonly string[]; service: Evidence["service"]; external?: readonly ExternalTest[] };
export type ExternalTest = { id: string; name: string; flag: string; service: string; requires?: string };
export type JUnitCase = { name: string; status: "passed" | "failed" | "skipped"; seconds: number };
export function parseJUnit(xml: string): JUnitCase[];
export const LIVE_TEST_FILES: readonly LiveTestFile[];

// tooling/src/opencode-acceptance/controls.ts
export type Control = { id: string; regression: string; check: string; apply: (stage: string) => void };
export function replaceOnce(file: string, from: string, to: string): void;
export const CONTROLS: readonly Control[];

// tooling/src/opencode-acceptance/budget.ts
export type BudgetSample = { kind: "reference" | "toolu"; spawnAt: number; requestTimes: number[] };
export function overheads(samples: readonly BudgetSample[]): { startupOverheadMs: number; perToolOverheadMs: number };

// tooling/src/opencode-host/provider.ts (changed)
export type RecordedRequest = { path: string; body: unknown; at: number };

// tooling/src/opencode-host/scenarios-entry.ts (changed)
export function acceptancePackageDir(env?: Record<string, string | undefined>): string; // TOOLU_ACCEPTANCE_PACKAGE ?? checkout package
```

`tools/toolu-opencode/contract/acceptance-budgets.json`:

```json
{ "version": 1, "startupOverheadMs": <ceiling>, "perToolOverheadMs": <ceiling> }
```

Report (`AcceptanceReportSchema`, zod, version 1):

```json
{
  "version": 1,
  "complete": true,
  "pass": true,
  "startedAt": "2026-10-04T20:00:00.000Z",
  "durationMs": 512345,
  "host": { "cli": "opencode-ai", "cliVersion": "1.18.34", "sdk": "@opencode-ai/plugin", "provisionedSdkVersion": "1.18.34",
            "platform": "darwin-arm64", "bun": "1.4.2", "installSource": "npm:opencode-ai@1.18.34 (bun add --exact)" },
  "tools": { "git": "…", "npm": "…", "ast-grep": "…", "agent-browser": "…", "chromium": "…" },
  "checks": [ { "id": "entry.npm-root", "family": "entry", "plugins": ["toolu"],
                "evidence": { "execution": "actual-host", "model": "scripted-loopback", "service": "none" },
                "status": "pass", "durationMs": 7012, "observed": {} } ],
  "controls": [ { "id": "control.missing-post-tool", "check": "posttool.shell", "detected": true, "observed": {} } ],
  "budget": { "startupOverheadMs": { "measured": 0, "ceiling": 0 }, "perToolOverheadMs": { "measured": 0, "ceiling": 0 }, "samples": [] },
  "external": [ { "id": "external.context7", "service": "context7.com", "execution": "in-process", "status": "available" } ],
  "coverage": { "agent-browser": ["browser.enabled", "…"] }
}
```

The `external[].status` values are `available` (passed), `unavailable` (failed or timed out) and `not-configured` (required key absent).

`pass` is true only when all of these hold:

- `complete` is true;
- every check passed;
- every control was detected;
- the budget passed;
- every catalog plugin has at least one passing dedicated check, one whose `plugins` is not `"all"`.

## Failure modes and edge cases

| Case | Behavior |
|---|---|
| Pinned CLI absent and the registry unreachable | Preflight fails with the `bun add` error; exit 1, no checks run |
| `TOOLU_OPENCODE_HOST_BIN` reports another version | Preflight fails with the pin mismatch |
| `ast-grep`, `agent-browser`, Chromium, `npm`, `git` or `tar` missing | Preflight lists each missing tool; exit 1. No check is reported as skipped |
| A scenario throws `ContractError` (harness precondition) | That check fails with the message; the run continues and exits 1 |
| A host run hangs | The existing `RUN_TIMEOUT_MS` guard kills it; the check fails |
| A live test file has a skipped host test, or names a test the registry does not expect | The check fails, listing the test names |
| context7.com unreachable | `external.context7` is `unavailable`, and acceptance is unaffected |
| `TYPESAFE_API_KEY` absent | `external.jev` is `not-configured`, and acceptance is unaffected |
| Committed probe results drift (host behavior changed) | Each drifted probe fails, with the diff line |
| A control's edit target moved | `replaceOnce` throws, the control fails (not detected), and the hermetic test fails earlier in `bun run test` |
| A control's check passes on the broken stage | The control is `detected: false`; exit 1 |
| Budget ceiling exceeded on a noisy runner | The budget fails with measured values and raw samples; rerunning is the remedy, and a raised ceiling needs a commit |
| `--only` used | The report has `complete: false` and `pass: false`; the exit code covers only the selected checks. A control selected without its check runs that check itself |
| Unknown `--only` id | `ContractError` before any host run; exit 1 |
| Concurrent runs both claim P's state | Locked atomic gate writes are expected to hold; a corrupted or failing P gate fails `concurrent.sessions` |
| CI: `gate` fails, or `opencode` is cancelled or skipped | The `typescript` aggregator fails (`if: always()` keeps it from being skipped) |
| Release-only diff | The workflow is skipped by `paths-ignore`, as today |

## Acceptance criteria

- **AC-1:** For each of the four regressions (a V2 entry, a `--` skill name, a missing `experimental.chat.system.transform` hook, a missing `tool.execute.after` hook), applied to a staged copy of the package, the matching acceptance check fails on the pinned host, and the run reports the control as `detected: true`. With the checkout's own package, the same four checks pass.
- **AC-2:**
  - **Deny:** for every denied tool kind on the pinned host, nothing changes. Each kind is checked in the report:

    | Denied tool kind | Check | Observation |
    |---|---|---|
    | write and edit | `pretool.files`, `entry.*` | `.env` byte-identical |
    | multi-file `apply_patch` | `pretool.patch` | `.env` unchanged, destination absent |
    | unsafe bash, commit and push | `pretool.shell`, `pretool.push` | marker absent |
    | MCP | `pretool.mcp` | the server's marker absent |
    | task child | `pretool.task` | the child's marker absent |
    | a refused commit | `concurrent.sessions` | no commit added |
  - **Allow:** an allowed bash call runs and leaves its marker (`pretool.baseline`, `pretool.files` `allowedBashRan`, `entry.*` `bashRan`).
  - **Post-edit quality:** an edit or write that leaves a quality violation, or a failing quality command, makes a later `git commit` and `git push` refused, with their markers absent.

  The acceptance report shows these as passing checks: `entry.*` and `concurrent.sessions` (deny and allow); `posttool.edit`, `tsquality.edit`, `pyquality.edit` and `rsquality.edit` (`commitDenied`, `pushDenied`, `markersAbsent`); `posttool.shell`.
- **AC-3:** The report's `coverage` gives each of the 16 catalog plugins at least one passing dedicated actual-host check. In CI the run happens on linux-x64 and darwin-arm64. When a required tool is removed from PATH, the run exits 1 with a preflight error naming that tool, and no check reports `skip`.
- **AC-4:** Every report check carries `evidence.execution`, `evidence.model` and `evidence.service`. External services appear only under `external`, with status `available`, `unavailable` or `not-configured`. The Markdown summary prints the three categories separately.
- **AC-5:** The report records the CLI version, provisioned SDK version, platform, Bun and tool versions, per-check durations, and the measured startup and per-tool overheads against the committed ceilings. An overhead above its ceiling fails the run.
- **AC-6:** In `bun run test:conformance`, `live-opencode` and `permission-evaluate` are gone and no suite can skip.
- **AC-7:** `.github/workflows/tests.yml` runs `bun run test:opencode` on `ubuntu-latest` and `macos-latest`. The job named `typescript` fails unless both the `bun run test` gate job and both acceptance jobs succeed. A hermetic test asserts this structure.
- **AC-8:** `permissions.other-after` and `permissions.other-before` pass on darwin-arm64, and their `loadOrder` observation reflects the order they arranged.
- **AC-9:** The full `bun run test:opencode` passes on darwin-arm64 locally and on both CI platforms for the PR head.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Staged package copies on pinned `opencode-ai@1.18.34` | Four `detected: true`; the same checks pass on the checkout | An edit target missing makes `replaceOnce` throw (hermetic) | `bun run test:opencode --only control.v2-entry control.invalid-skill-name control.missing-context control.missing-post-tool entry.local-shim surface.discovered-names live.context-delivery posttool.edit` (exit 0 only when all eight pass, i.e. four controls detected and four checks pass); `bun test tooling/src/opencode-acceptance/__tests__/controls.test.ts` |
| AC-2 | Isolated projects with `.env`, gate state and a scripted model | Deny, allow and post-quality observations true | Three concurrent runs over two projects | `bun run test:opencode` (report checks) |
| AC-3 | The catalog from `listPluginManifests`, and the registry | All 16 plugins covered; a removed tool fails preflight | A `PATH` without `agent-browser`, set inside the test | `bun test tooling/src/opencode-acceptance/__tests__/coverage.test.ts tooling/src/opencode-acceptance/__tests__/preflight.test.ts` |
| AC-4 | A real `bun test --reporter=junit` run of a fixture file with pass, skip and fail cases; real report output | Evidence fields present; external apart | A key absent gives `not-configured` | `bun test tooling/src/opencode-acceptance/__tests__/live-tests.test.ts tooling/src/opencode-acceptance/__tests__/report.test.ts` |
| AC-5 | Recorded request timestamps from real budget runs | Overheads computed; the ceiling decides | A sample with a missing tool request fails | `bun test tooling/src/opencode-acceptance/__tests__/budget.test.ts`; `bun run test:opencode` |
| AC-6 | The conformance matrix | All suites `pass`; no `skip` status type | — | `bun run test:conformance`; `bun test tools/toolu-conformance` |
| AC-7 | `.github/workflows/tests.yml` parsed with `Bun.YAML` | Matrix OSes, command, and aggregator needs/if/guard | Removing macOS from the matrix fails the test | `bun test tooling/src/opencode-acceptance/__tests__/workflow.test.ts` |
| AC-8 | Pinned host on APFS | `loadOrder: true` both ways | — | `bun run smoke:opencode-permissions` |
| AC-9 | Full run | `pass: true` | — | `bun run test:opencode`; CI `opencode (ubuntu-latest)`, `opencode (macos-latest)` |

## Documentation impact

- `docs/conformance-report.md`: remove the V2 suites and the optional live lane; describe the mandatory OpenCode acceptance, its evidence categories, its platforms and the latest recorded run.
- `docs/opencode-host-contract.md`: platforms probed (linux-x64 and darwin-arm64, both in CI); the CI paragraph becomes mandatory acceptance; the Reproduce section adds `bun run test:opencode`. This is prose outside the generated markers.
- `docs/opencode.md`: replace the `TOOLU_LIVE_OPENCODE` optional-lane text with `bun run test:opencode`, keeping the per-file live-test notes.
- `docs/testing.md`: the CI jobs (`gate`, `opencode`, the `typescript` aggregator).
- `docs/shell-analysis.md`, `docs/conventions-adoption.md`: job-name references.
- `docs/portable-core.md`: the V2 adapter is driven only by the clean-install smoke.
- `AGENTS.md`: CI table rows, plus a key-files row for `tooling/src/opencode-acceptance.ts`.

## Open Questions

- **Exact budget ceilings.** Owner: this delivery. Non-blocking: they are calibrated from the first local and CI measurements, with at least a 3× margin, before the PR is ready.
- **Admin adds `opencode (*)` as separate required contexts.** Owner: repository admin. Non-blocking: the `typescript` aggregator already makes acceptance required.

## Review

Spec review, 2026-10-04. Jev's judgments on requirement–evidence alignment, after revision: I1 0.82, I2 0.83, I3 0.78, I4 0.85, scope 0.94.

- Acceptance criteria: 🟡 should-fix: AC-2 named only the post-quality *command* path, while the issue asks for a post-*edit* failure. Fixed: added `posttool.edit` and the three `*quality.edit` checks, and moved `control.missing-post-tool` to `posttool.edit`.
- Acceptance criteria: 🟡 should-fix: the deny proof did not enumerate tool kinds. Fixed: the AC-2 table maps every denied kind to its check and observation.
- Architecture: 🟡 should-fix: the issue's scope items had no explicit mapping. Fixed: the "Issue scope → checks" table.
- Architecture: 🔵 consider: a narrowed run's exit code made the AC-1 ledger check unusable. Fixed: the exit code covers the selected checks, the report keeps `pass: false`, and the workflow test forbids `--only` in CI.
- Non-Goals: 🔵 consider: the harness edits were understated. Fixed: all four are listed.
