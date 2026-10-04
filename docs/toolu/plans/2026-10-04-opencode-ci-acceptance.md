# OpenCode CI acceptance (OP-28) — Plan

**Date:** 2026-10-04 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-04-opencode-ci-acceptance-design.md **Topic:** One mandatory `bun run test:opencode` run on Linux and macOS that proves every catalog plugin on the pinned host, with evidence categories, budgets and regression controls

## Evidence and approach

Inspected at `24e65189`:

- **Existing scenario arrays.** `tooling/src/opencode-entry-smoke.ts` (the `ALL_SCENARIOS` composition), `opencode-host/smoke-main.ts` and `pretool-shared.ts` (`runPretoolScenarios`, `session`), `opencode-host-probe.ts` (`warmUp`, `runScenarios`, `resultDrift`) and `opencode-surface-probe.ts` (top-level `main`).
- **Seven live test files.** `tools/toolu-opencode/src/plugin/__tests__/*.live.test.ts`, gated by `TOOLU_LIVE_OPENCODE`. External variants use `TOOLU_LIVE_CONTEXT7` (keyless) and `TOOLU_LIVE_JEV` (`TYPESAFE_API_KEY`).
- **Packaging and the host.**
  - `stageOpencode` (`tooling/src/npm-pack.ts`) copies the package except `plugins/` and `node_modules/`.
  - `installShim` (`scenarios-entry.ts`) and `session` (`pretool-shared.ts`) hard-code the checkout package directory.
  - The scripted provider (`provider.ts`) records `{path, body}`.
  - The host's `createTooluHooks` (`tools/toolu-opencode/src/plugin/hooks.ts`) returns the `"tool.execute.after"` and `"experimental.chat.system.transform"` keys, each exactly once.
- **Conformance.** `CONFORMANCE_SUITES` (`tools/toolu-conformance/src/cli/matrix.ts`) holds `live-opencode` and `permission-evaluate`, and `run.test.ts` asserts the live skip.
- **Repository rules.**
  - Guardrails (`tooling/guardrails.config.json`) whitelist `tooling/src` top-level folders and cap functions at 60 lines.
  - jscpd runs at threshold 0, so no copied scaffolding.
  - An import alias trips the text guard.
  - Knip discovers script entries from root `package.json`.
- **Baseline on darwin-arm64.** 8 minutes in total, with every family passing except `permissions.other-after`. A scratch probe confirmed that config-listed plugins precede `.opencode/plugins/*` on this host.
- **Local quirks** (memory). Run gates with `env -u npm_config_store_dir TMPDIR=/private/tmp`, and admit expensive jobs through the epic `job.ts` slot.

Approach (from the approved spec): one in-process runner over the existing arrays, a typed registry with evidence and plugin coverage, JUnit-parsed live test files, three new live checks, four staged regression controls, the deterministic order fix, conformance cleanup, a CI matrix plus a `typescript` aggregator, and docs.

## Workstream summary

Harness hooks → order fix → conformance cleanup → runner core → new live checks → controls → full local run → CI → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "harness-hooks",
    "title": "Stamp scripted-provider requests with arrival time, let TOOLU_ACCEPTANCE_PACKAGE override the shimmed package directory in installShim and pretool-shared session, and move the generated-surface probe into an exported function behind its unchanged script",
    "ac_refs": ["AC-1", "AC-5"],
    "paths": ["tooling/src/opencode-host/**", "tooling/src/opencode-surface-probe.ts", "tools/toolu-opencode/generated/**"],
    "input": "A real loopback provider receiving two POSTs; acceptancePackageDir with and without TOOLU_ACCEPTANCE_PACKAGE set to a temp dir; the committed generated/ tree on the pinned host",
    "check": "bun test --timeout 60000 tooling/src/opencode-host/__tests__ && bun run probe:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "permission-order",
    "title": "Order the other-denier plugin by load route (opencode.json plugin entries before .opencode/plugins) so permissions.other-before and other-after arrange their order deterministically on APFS and ext4",
    "ac_refs": ["AC-8"],
    "depends_on": ["harness-hooks"],
    "paths": ["tooling/src/opencode-host/scenarios-permissions-advice.ts", "tooling/src/opencode-host/pretool-shared.ts", "tooling/src/opencode-host/session.ts", "tools/toolu-opencode/src/**", "tools/toolu-opencode/contract/probes/probe.ts"],
    "input": "The pinned opencode-ai 1.18.34 on darwin-arm64 in isolated profiles with the scripted provider",
    "check": "bun run smoke:opencode-permissions",
    "model": "inherit"
  },
  {
    "id": "conformance",
    "title": "Delete the live-opencode and permission-evaluate conformance suites, remove skip from SuiteOutcome, assert every suite passes, and drop helpers knip reports unused",
    "ac_refs": ["AC-6"],
    "paths": ["tools/toolu-conformance/**", "tools/toolu-opencode/src/adapter/**"],
    "input": "The real conformance matrix over protected-files, bootstrap-readiness, surface-drift and spaces-cwd fixture projects",
    "check": "bun test --timeout 120000 tools/toolu-conformance && bun run test:conformance && bun run knip",
    "model": "inherit"
  },
  {
    "id": "runner-core",
    "title": "Add bun run test:opencode: preflight (host and tools), warm-up, the check registry with plugins and evidence, JUnit live-test runner with external tests apart, report schema, Markdown summary, coverage verdict and --only; whitelist the folder in guardrails",
    "ac_refs": ["AC-3", "AC-4"],
    "depends_on": ["harness-hooks", "conformance"],
    "paths": ["tooling/src/opencode-acceptance.ts", "tooling/src/opencode-acceptance/**", "tooling/src/opencode-host/**", "tooling/src/opencode-*-smoke.ts", "tooling/guardrails.config.json", "tools/toolu-opencode/src/plugin/__tests__/*.live.test.ts", "plugins/*/.claude-plugin/plugin.json", "package.json"],
    "input": "A real bun test --reporter=junit run of a fixture test file with passing, skipped and failing tests; the catalog from listPluginManifests; every *.live.test.ts on disk; preflight with a PATH that lacks agent-browser (real binaries otherwise); a report built from real check outcomes",
    "check": "bun test --timeout 120000 tooling/src/opencode-acceptance/__tests__ && bun run guardrails",
    "model": "inherit"
  },
  {
    "id": "new-checks",
    "title": "Add live surface.discovered-names, concurrent.sessions and budget.overhead (differential, ceilings in contract/acceptance-budgets.json) and register them",
    "ac_refs": ["AC-2", "AC-5"],
    "depends_on": ["runner-core"],
    "paths": ["tooling/src/opencode-acceptance/**", "tooling/src/opencode-host/**", "tools/toolu-opencode/contract/acceptance-budgets.json", "tools/toolu-opencode/src/**", "tools/toolu-opencode/generated/**", "plugins/**", "packages/toolu-core/src/**"],
    "input": "The pinned host with toolu shimmed over all 16 plugins; two concurrent runs in one project plus a third project with a seeded failing gate; reference runs with a no-op plugin; recorded request timestamps",
    "check": "bun test --timeout 60000 tooling/src/opencode-acceptance/__tests__/budget.test.ts && bun run test:opencode --only surface.discovered-names concurrent.sessions budget.overhead",
    "model": "inherit"
  },
  {
    "id": "controls",
    "title": "Add the four staged regression controls (V2 entry, -- skill name, missing system-context hook, missing tool.execute.after) with replaceOnce edits, a hermetic edit test, and live detection",
    "ac_refs": ["AC-1"],
    "depends_on": ["new-checks"],
    "paths": ["tooling/src/opencode-acceptance/**", "tooling/src/opencode-host/**", "tooling/src/npm-pack.ts", "tools/toolu-opencode/src/**", "tools/toolu-opencode/generated/**", "tools/toolu-opencode/src/plugin/__tests__/context-delivery.live.test.ts", "plugins/**", "packages/toolu-core/src/**"],
    "input": "Fresh stages of tools/toolu-opencode from the checkout; the pinned host running entry.local-shim, surface.discovered-names, live.context-delivery and posttool.edit against each stage and against the checkout",
    "check": "bun test --timeout 120000 tooling/src/opencode-acceptance/__tests__/controls.test.ts && bun run test:opencode --only control.v2-entry control.invalid-skill-name control.missing-context control.missing-post-tool entry.local-shim surface.discovered-names live.context-delivery posttool.edit",
    "model": "inherit"
  },
  {
    "id": "full-local",
    "title": "Run the complete acceptance on darwin-arm64, calibrate the budget ceilings to at least 3x the measured overheads, and keep the report",
    "ac_refs": ["AC-2", "AC-3", "AC-4", "AC-5", "AC-9"],
    "depends_on": ["permission-order", "controls"],
    "paths": ["tooling/src/**", "tools/toolu-opencode/**", "plugins/**", "packages/toolu-core/src/**", "package.json"],
    "input": "The full registry on the pinned host in isolated profiles; the agent-browser, ast-grep, git, npm and tar installed on this machine",
    "check": "bun run test:opencode --report /private/tmp/oc362/opencode-acceptance-darwin-arm64.json",
    "model": "inherit"
  },
  {
    "id": "ci",
    "title": "Rename the heavy job to gate (bun run test), add the opencode matrix job (ubuntu-latest, macos-latest) with tools, cache and report artifact, and the typescript aggregator; assert the structure hermetically",
    "ac_refs": ["AC-7", "AC-3"],
    "depends_on": ["runner-core"],
    "paths": [".github/workflows/tests.yml", "tooling/src/opencode-acceptance/__tests__/workflow.test.ts", "package.json"],
    "input": "The real .github/workflows/tests.yml parsed with Bun.YAML",
    "check": "bun test --timeout 60000 tooling/src/opencode-acceptance/__tests__/workflow.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the mandatory acceptance in conformance-report, opencode-host-contract (platforms, CI, reproduce), opencode.md, testing.md, shell-analysis.md, conventions-adoption.md, portable-core.md and AGENTS.md",
    "ac_refs": ["AC-3", "AC-4", "AC-6", "AC-7"],
    "depends_on": ["full-local", "ci"],
    "paths": ["docs/**", "AGENTS.md", "tools/toolu-opencode/README.md", "tools/toolu-opencode/contract/**", "tooling/src/check-portable-core-doc.ts", "tooling/src/opencode-host-contract.ts", "tooling/src/opencode-host/**"],
    "input": "The local acceptance report and the final workflow",
    "check": "test -z \"$(grep -lE 'live-opencode|permission-evaluate|TOOLU_LIVE_OPENCODE' docs/conformance-report.md docs/opencode-host-contract.md AGENTS.md docs/testing.md)\" && grep -q 'test:opencode' docs/conformance-report.md && grep -q 'test:opencode' docs/opencode-host-contract.md && grep -q 'test:opencode' docs/opencode.md && grep -q 'test:opencode' docs/testing.md && grep -q 'test:opencode' AGENTS.md && bun run test:portable-core",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8", "AC-9"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "The whole branch diff with the focused, live and hermetic checks green",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

**Create:**

- `tooling/src/opencode-acceptance.ts`, the CLI entry;
- under `tooling/src/opencode-acceptance/`: `checks.ts`, `families.ts` (registry data), `live-tests.ts`, `preflight.ts`, `report.ts`, `budget.ts`, `controls.ts`, `run.ts`, and `__tests__/{coverage,live-tests,preflight,report,budget,controls,workflow}.test.ts` plus `__tests__/fixtures/junit-sample.test.ts`;
- `tooling/src/opencode-host/scenarios-acceptance.ts` and `surface-probe.ts`;
- `tools/toolu-opencode/contract/acceptance-budgets.json`.

**Modify:**

- `tooling/src/opencode-host/provider.ts`, `scenarios-entry.ts`, `pretool-shared.ts` and `scenarios-permissions-advice.ts`;
- `tooling/src/opencode-surface-probe.ts`;
- `tooling/guardrails.config.json`;
- root `package.json` (`test:opencode`);
- `.github/workflows/tests.yml`;
- `tools/toolu-conformance/src/cli/{matrix,types}.ts`, `__tests__/run.test.ts`, and the deleted `suites/live-opencode.ts` and `suites/permission-evaluate.ts`;
- the docs listed in the spec.

## Verification

- **Real inputs only.** Use the pinned `opencode-ai@1.18.34` binary, isolated HOME/XDG profiles, temp git projects, the loopback scripted provider and fixture servers, staged package copies for the controls, a real `bun test --reporter=junit` run, and the real workflow file. Nothing is mocked.
- **Boundaries:**
  - a missing tool on PATH;
  - a skipped or missing JUnit test;
  - an absent `TYPESAFE_API_KEY` (`not-configured`);
  - three concurrent host runs over two projects;
  - an edit target that moved, which makes `replaceOnce` throw;
  - an undetected control;
  - `--only`, which yields `complete: false`;
  - a budget sample missing a request.
- **Show the suite can fail.** The four controls are the standing proof. While developing, also re-run `concurrent.sessions` with Q's gate file left absent and confirm it fails, then restore.
- **Gates.** Run focused checks, the three live narrowed runs, the full `bun run test:opencode`, `env -u npm_config_store_dir TMPDIR=/private/tmp bun run test` through the job slot, the ledger `run --verify`, the review, and verdict readiness before push.
- **CI.** Both `opencode (ubuntu-latest)` and `opencode (macos-latest)` must pass for the PR head, along with the `typescript` aggregator.
- **Rebase.** Rebase onto `origin/main` before implementation and before pushing if it moved, then re-run affected steps.

## Plan review

**Status:** Approved.

- **Coverage.** Every spec AC (AC-1 to AC-9) maps to at least one step, with no dangling ids, checked by comparing the ledger's `ac_refs` with the spec's bold ids.
- **Order.** The order is acyclic: harness-hooks → permission-order, and conformance → runner-core → new-checks → controls → full-local; runner-core → ci; then docs → gate.
- **Jev, step–requirement alignment.** AC-1 0.87, AC-2 0.79, AC-3 0.86, AC-4 0.80, AC-5 0.82, AC-6 0.85, AC-7 0.81, AC-8 0.76, AC-9 0.26; order 0.81.

Findings:

- AC-9: 🟡 should-fix: the CI half (`opencode (ubuntu-latest)` and `opencode (macos-latest)` green on the PR head) cannot be a pre-push ledger check, because CI runs only after the push. Resolved: the ledger proves the darwin-arm64 half (`full-local`). The CI half is post-push evidence that `pr-babysit` gates on (CI green), and it is recorded in the PR before `report ready`.
- docs: 🔵 consider: `grep -l` with `\|` is not portable to BSD grep. Fixed: `grep -lE`.
- full-local: 🔵 consider: the report directory must exist. Resolved: the runner creates the report's parent directory.

## Deviations

- **Concurrent sessions use a second profile.** Two `opencode run` processes that start on the same profile at the same moment fail in the host with `database is locked`, because the host keeps its session database in the profile. That is a host limit, not toolu's. P1 and P2 therefore share project P (its gate files and toolu state) but use separate profiles, which models a second user or terminal. Q stays a separate project.
- **The invalid-skill-name control is caught twice.** toolu validates its own catalog at startup, refuses the double-hyphen id (`toolu: not ready: surfaces: invalid … an ID is lowercase words joined by single hyphens`) and denies every tool call. `surface.discovered-names` therefore fails on `ready: 0` and every skill missing, before its name rule is reached. An unmodified stage passes the same check (18/18 skills owned), so the detection is not a staging artifact. The check now records `ready` and requires it.
- **Budget in the check, not a top-level report key.** `budget.overhead` is an ordinary check, so `--only` and coverage treat it like any other. Its observations carry the measured overheads, the ceilings and raw request offsets. First darwin-arm64 measurement: +444 ms startup and +45 ms per tool call. The ceilings are 6000 ms and 1500 ms, at least 3× any measurement so far; they are re-checked against the CI Linux numbers before ready.
- **Service credentials set aside.** The runner removes `CONTEXT7_API_KEY`, `EXA_API_KEY`, `TYPESAFE_API_KEY` and the `JIRA_*` credentials from its own environment before any check, so a check labeled `fixture` cannot quietly reach a live service. Only the matching external test gets its key back. The report lists what was set aside (`scrubbed`).
- **`--only` names checks or controls.** A selected control runs its check against its stage even when that check is not selected. Selection happens before preflight, so a typo fails without a host run. The CLI test proves this through `bun run test:opencode --only nope.missing`.
- **Preflight test without real tools.** The gate job (`bun run test`) does not install `agent-browser`, so `preflight.test.ts` builds its own `PATH` from `git`/`tar` links and controlled stub executables (a broken `ast-grep`, an `agent-browser` whose doctor reports no Chromium) instead of depending on this machine's tools.
- **Probe drift is per probe.** Each contract probe is its own check (`probe.<id>`), and `probe.host-versions` compares the CLI and provisioned SDK versions. A drifted probe fails with both observations, instead of failing the run once.
- **No `wip:` push gate.** The toolu push gate (fresh ledger plus review) blocks intermediate pushes, so the branch keeps its commits locally until delivery. The orchestrator snapshots `refs/epic-wip/*`.
- **Review triage.** An independent review found 5 should-fix items and 5 nits. Fixed:
  - **Controls need a pristine pass.** A control now has to pass against the unbroken stage before its edit; Jev chose this 0.99 over per-control markers. Staging fails on a missing dependency link.
  - **Secrets.** External-test output is redacted of the key it received.
  - **Budget readiness.** The ready string follows the catalog size.
  - **Run setup.** A preset `TOOLU_ACCEPTANCE_PACKAGE` is refused, and the pack directory is cleaned when packing fails.
  - **New tests:** live-test verdicts (a skipped host test fails), external statuses, control verdicts, credential set-aside, selection, and a workflow test that rules out a skip condition on the gate or acceptance.
  - **Small fixes:** two stale comments, and the conformance helpers' unused `.opencode` variant.

  Declined: switching the aggregator from `always()` to `!cancelled()`. A cancelled run would then leave the required `typescript` status skipped, which GitHub counts as passing; `always()` makes it fail (Jev 0.91).
