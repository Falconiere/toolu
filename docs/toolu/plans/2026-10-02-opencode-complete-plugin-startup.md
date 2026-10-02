# OpenCode complete plugin startup (OP-08) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-complete-plugin-startup-design.md   **Topic:** Every selected plugin's `hooks.json` SessionStart entries run in dependency order; readiness comes from this run's verified records; disabled plugins' owned contributions are removed.

## Evidence and approach

**Recalled.**
- `6990a41a`: the OP-01 host facts, including fail-open init and the headless driving rules.
- `ce014dd7`: the epic's corrective target, which identifies the bootstrap's register-OR-session-start choice as a defect.

**Inspected.**
- `tools/toolu-opencode/src/bootstrap/{entrypoint,runtime,readiness,result}.ts` and their two test files.
- `src/plugin/{enforcement,hooks}.ts`.
- `src/select/resolve.ts`, kept unchanged because of `generate-surface` ordering.
- `packages/toolu-core/src/registry/registry-register.ts` (`runRegisterHook`, `registerModules`).
- `packages/toolu-core/src/startup/{publish,context}.ts`.
- `packages/toolu-core/src/launcher/launcher.ts` (`launcherCommand`).
- `plugins/toolu/hooks/src/lifecycle/session-housekeeping.ts`, the marker writer.
- Every plugin's `hooks/hooks.json` SessionStart entries.
- `tooling/src/check-hooks-json.ts`: the plugin name is the directory name.
- `tools/toolu-opencode/scripts/bundle-plugins.ts`: the `.requires-native-register` staging.
- `tools/toolu-conformance/src/cli/suites/bootstrap-readiness.ts`.
- The live harness `tooling/src/opencode-host/scenarios-entry.ts` and `tooling/src/opencode-entry-smoke.ts`.
- The pinned host binary in `~/.cache/toolu/opencode-host/1.18.34`.
- Guardrails: functions ≤60 lines, and the `nested` list in `tools/toolu-opencode/guardrails.config.json` restricts subdirectories only. Also knip and jscpd.

**Approach.** The spec's architecture. The core gets one new module, `startup/report.ts`, and two callers, followed by a rebuild of the affected plugin bundles. The OpenCode bootstrap is split into focused files:

- `entrypoint.ts`: entries from `hooks.json`.
- `order.ts`: topological order.
- `spawn.ts`: the bounded child with deadline and abort.
- `output.ts`: strict hook stdout.
- `records.ts`: report parsing and on-disk verification.
- `ledger.ts`: ownership and prune.
- `runtime.ts`: orchestration.
- `readiness.ts`: aggregation into the result.

## Workstream summary

1. Core report channel, with tests and bundle rebuild.
2. Marker removal in toolu.
3. Bootstrap: entries and order, then spawn, output and records, then ledger, then runtime and result.
4. Enforcement and log wiring.
5. Fixtures and real-data tests: catalog, readiness, ledger, failure, order, conformance.
6. Remove `bundle-plugins` marker staging.
7. Live smoke scenarios on the pinned host.
8. Docs, then the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "core-report",
    "title": "packages/toolu-core/src/startup/report.ts: STARTUP_REPORT_ENV, StartupRecordSchema, reportStartup (append JSONL when TOOLU_STARTUP_REPORT is set; stderr + exitCode 1 on append failure); export from @toolu/core/startup; runRegisterHook reports one registry record per module plus error records; publishWrapper reports a helper record; colocated subprocess tests",
    "ac_refs": ["AC-7"],
    "paths": [
      "packages/toolu-core/src/startup/**",
      "packages/toolu-core/src/registry/registry-register.ts",
      "packages/toolu-core/src/registry/__tests__/**"
    ],
    "input": "Real temp config roots; real bundles (one present, one deleted) passed to runRegisterHook in a Bun subprocess with and without TOOLU_STARTUP_REPORT; publishWrapper with a present source, a missing source and a user regular file; an unwritable report path (a directory)",
    "check": "bun test --timeout 60000 -t '^(?!.*(refuses the link|unreadable .* file fails)).*$' packages/toolu-core/src/startup/__tests__ packages/toolu-core/src/registry/__tests__",
    "model": "inherit"
  },
  {
    "id": "marker",
    "title": "Stop touching toolu/.session-start-ready in session-housekeeping.ts; update the session-start test assertion and the housekeeping comment",
    "ac_refs": ["AC-2"],
    "paths": [
      "plugins/toolu/hooks/src/lifecycle/session-housekeeping.ts",
      "plugins/toolu/hooks/src/__tests__/session-start.test.ts",
      "plugins/toolu/hooks/src/lifecycle/session-notices.ts"
    ],
    "input": "The real toolu session-start bundle in a sandboxed Claude host",
    "check": "bun test --timeout 60000 plugins/toolu/hooks/src/__tests__/session-start.test.ts",
    "model": "inherit"
  },
  {
    "id": "bundles",
    "title": "Rebuild plugin bundles that embed the changed core/toolu sources (bun run build:plugins) and confirm Claude/Codex plugin startup tests stay green",
    "ac_refs": ["AC-7"],
    "depends_on": ["core-report", "marker"],
    "paths": [
      "plugins/*/hooks/dist/**",
      "plugins/*/hooks/src/**",
      "packages/toolu-core/src/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "Committed plugin sources and the pinned Bun",
    "check": "bun run check:plugin-bundles && bun test --timeout 120000 -t '^(?!.*(refuses the link|unreadable .* file fails)).*$' plugins/agent-browser/hooks/src/__tests__/session-start.test.ts plugins/ast-grep/hooks/src/__tests__/register.test.ts plugins/context7/hooks/src/__tests__/session-start.test.ts plugins/epic-orchestrator/hooks/src/__tests__/check-deps.test.ts plugins/exa-search/hooks/src/__tests__/session-start.test.ts plugins/jev/hooks/src/__tests__/session-start.test.ts plugins/jira/hooks/src/__tests__/session-start.test.ts plugins/pr-babysit/hooks/src/__tests__/check-toolu.test.ts plugins/python-quality/hooks/src/__tests__/register.test.ts plugins/rust-quality/hooks/src/__tests__/register.test.ts plugins/statusline/hooks/src/__tests__/session-start.test.ts plugins/toolu-review/hooks/src/__tests__/session-start.test.ts plugins/toolu/hooks/src/__tests__/session-start.test.ts plugins/ts-quality/hooks/src/__tests__/register.test.ts",
    "model": "inherit"
  },
  {
    "id": "entries-order",
    "title": "bootstrap/entrypoint.ts pluginStartupEntries (strict hooks.json schema, startup matcher, exact launcherCommand, bundle presence) and bootstrap/order.ts startupOrder (alphabetical DFS, deps first, cycle path, unselected dependency); colocated tests",
    "ac_refs": ["AC-1", "AC-2", "AC-6"],
    "paths": [
      "tools/toolu-opencode/src/bootstrap/entrypoint.ts",
      "tools/toolu-opencode/src/bootstrap/order.ts",
      "tools/toolu-opencode/src/bootstrap/__tests__/startup-entries.test.ts",
      "tools/toolu-opencode/src/bootstrap/__tests__/startup-order.test.ts",
      "tools/toolu-opencode/src/bootstrap/__tests__/fixtures.ts",
      "plugins/*/hooks/hooks.json",
      "plugins/*/.claude-plugin/plugin.json"
    ],
    "input": "Every real plugin directory (ts-quality → register + check-toolu, brainstorm → none); fixture plugins with a hand-written command, a missing bundle, a compact-only matcher, invalid JSON; the real delivery-flow closure; a two-plugin cycle",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/bootstrap/__tests__/startup-entries.test.ts tools/toolu-opencode/src/bootstrap/__tests__/startup-order.test.ts",
    "model": "inherit"
  },
  {
    "id": "bootstrap-core",
    "title": "bootstrap/spawn.ts (bounded stdout/stderr, deadline, AbortSignal, startup stdin, report file env), output.ts (strict SessionStart hook JSON, bounded context), records.ts (report parse + on-disk verification with realpath containment), ledger.ts (read/write/prune owned contributions), readiness.ts (aggregate per-plugin outcomes into ReadyResult/NotReady reason), result.ts types, runtime.ts orchestration with dependency-failure skipping; index.ts exports",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["bundles", "entries-order"],
    "paths": [
      "tools/toolu-opencode/src/bootstrap/**",
      "tools/toolu-opencode/src/host/roots.ts",
      "tools/toolu-opencode/src/inventory/**",
      "tools/toolu-opencode/src/preflight/check.ts",
      "plugins/*/hooks/dist/**",
      "plugins/*/hooks/hooks.json",
      "packages/toolu-core/src/**"
    ],
    "input": "The real 16-plugin catalog; temp copies of ast-grep/context7/ts-quality/toolu with one bundle deleted; pre-seeded stale marker, unrelated module and old-version module; fixture bundles that exit 7, stall, flood stdout, print non-JSON, write invalid or foreign records; selection rewritten between runs; the same project bootstrapped twice (bytes, mtimes, ledger compared); a user file at a helper path; an AbortController aborted mid-entry",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/bootstrap/__tests__",
    "model": "inherit"
  },
  {
    "id": "enforcement",
    "title": "prepareEnforcement passes AbortSignal.timeout(180_000) and returns plugins/diagnostics; createTooluHooks logs `toolu: ready (<p> plugins, <n> startup artifacts)` plus capped diagnostics; deny-all message carries the per-plugin reason",
    "ac_refs": ["AC-5"],
    "depends_on": ["bootstrap-core"],
    "paths": [
      "tools/toolu-opencode/src/plugin/**",
      "tools/toolu-opencode/src/bootstrap/**",
      "tools/toolu-opencode/src/adapter/**",
      "plugins/*/hooks/dist/**"
    ],
    "input": "Temp project with the real catalog (ready) and with a catalog copy missing ts-quality's post-tool-use.js (not ready); a real tool.execute.before call",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__ tools/toolu-opencode/src/adapter/__tests__ && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "staging-conformance",
    "title": "Drop .requires-native-register staging from bundle-plugins.ts and its test; rewrite the conformance bootstrap-readiness suite to the new acceptance (stale marker + unrelated module + missing declared bundle → not-ready)",
    "ac_refs": ["AC-2"],
    "depends_on": ["bootstrap-core"],
    "paths": [
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tooling/src/__tests__/bundle-plugins.test.ts",
      "tools/toolu-conformance/src/cli/suites/**",
      "tools/toolu-opencode/src/bootstrap/**"
    ],
    "input": "A legacy-only fixture plugin staged through stagePlugins; a fixture plugin whose hooks.json declares a missing register bundle with a pre-seeded marker and unrelated module",
    "check": "bun test --timeout 60000 tooling/src/__tests__/bundle-plugins.test.ts && bun run test:conformance",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Live smoke scenarios entry.full-startup (all 16 enabled via local shim → one ready line naming 16 plugins, expected modules/helpers on disk, bash allowed), entry.startup-failure (catalog copy without ts-quality post-tool-use.js → not ready naming ts-quality, bash refused, no file), entry.startup-disable (second session without ast-grep → no ast-grep modules)",
    "ac_refs": ["AC-8"],
    "depends_on": ["enforcement", "staging-conformance"],
    "paths": [
      "tooling/src/opencode-host/scenarios-entry.ts",
      "tooling/src/opencode-host/scenarios-startup.ts",
      "tooling/src/opencode-entry-smoke.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "plugins/*/hooks/**"
    ],
    "input": "Pinned opencode-ai@1.18.34 from the version-keyed cache, isolated profiles, scripted loopback provider, local shim with TOOLU_REPO_ROOT (repo or a temp catalog copy), .opencode/toolu/plugins.json selections",
    "check": "bun run smoke:opencode-entry",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/opencode.md bootstrap paragraph and scoped cleanup; docs/registry.md Registering paragraph (report records); AGENTS.md @toolu/core/startup row; tools/toolu-opencode/README.md if it describes bootstrap",
    "depends_on": ["live"],
    "paths": [
      "docs/opencode.md",
      "docs/registry.md",
      "AGENTS.md",
      "tools/toolu-opencode/README.md"
    ],
    "input": "The implemented behavior and live smoke output",
    "check": "! grep -n 'register.js. bundle, else\\|session-start-ready\\|bootstrap artifacts)' docs/opencode.md && grep -q 'TOOLU_STARTUP_REPORT' docs/registry.md && grep -q 'startup-ledger.json' docs/opencode.md",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate: bun run test (format, lint, typecheck, guardrails, knip, jscpd, unit, portable-core, gate coverage, bundles, hooks-json, workspace, pack, conformance, context budget, benchmarks, shell bench)",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "The whole repository at the branch head",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- **Create:**
  - `packages/toolu-core/src/startup/report.ts` and its test.
  - `tools/toolu-opencode/src/bootstrap/{order,spawn,output,records,ledger}.ts`.
  - Bootstrap tests `startup-{entries,order,catalog,readiness,ledger}.test.ts`, plus `__tests__/fixtures.ts`.
  - `tooling/src/opencode-host/scenarios-startup.ts`.
- **Modify:**
  - Core: `packages/toolu-core/src/startup/{startup,publish}.ts`, `packages/toolu-core/src/registry/registry-register.ts`.
  - toolu plugin: `plugins/toolu/hooks/src/lifecycle/session-housekeeping.ts` and `__tests__/session-start.test.ts`.
  - Rebuilt bundles: `plugins/*/hooks/dist/*.js`.
  - Bootstrap: `tools/toolu-opencode/src/bootstrap/{entrypoint,runtime,readiness,result,index}.ts` and the two existing bootstrap tests.
  - Plugin: `tools/toolu-opencode/src/plugin/{enforcement,hooks}.ts` and tests.
  - Staging and conformance: `tools/toolu-opencode/scripts/bundle-plugins.ts`, `tooling/src/__tests__/bundle-plugins.test.ts`, `tools/toolu-conformance/src/cli/suites/bootstrap-readiness.ts`.
  - Live smoke: `tooling/src/opencode-entry-smoke.ts`.
  - Docs: `docs/opencode.md`, `docs/registry.md`, `AGENTS.md`.

## Verification

- **AC-1:** the catalog test runs all 16 real plugins into a temp data root and asserts the exact module and helper sets, byte equality, order and contexts.
- **AC-2:** the readiness tests delete one bundle in temp catalog copies, with a stale marker, an unrelated module and an old-version module present.
- **AC-3:** two runs produce identical files, mtimes and ledger.
- **AC-4:** the ledger test switches the selection and covers a user file at a helper path.
- **AC-5:** fixture bundles cover every failure class, plus the deny-all message.
- **AC-6:** the real `delivery-flow` closure order and a fixture cycle.
- **AC-7:** core tests with and without the variable, plus the unchanged Claude-host plugin tests.
- **AC-8:** the live smoke on the pinned host.
- **Documentation:** synchronized in the `docs` step.
- **Gate:** `bun run test` must pass with zero failures or warnings.
- **Delivery:**
  1. Scoped conventional commits on `feat/342-opencode-run-complete-plugin-startup`.
  2. `plan-ledger.js run <plan> --verify` over the whole branch diff.
  3. `toolu-review:review` with version 2 push-review state.
  4. `verdict.js status` reports `overall: ready`.
  5. Push, then the PR to `main` (`Closes Falconiere/toolu#342`, `Part of Falconiere/toolu#334`), then `pr-babysit:babysit`.

## Plan review

- `bootstrap-core` input: 🟡 should-fix (fixed). AC-3 (a repeated run) and AC-5 (abort) were not named in the step input; both are now listed.
- Verification: 🟡 should-fix (fixed). The delivery sequence was implicit; it is now stated.
- AC coverage: every spec AC (1–8) is mapped and no ref dangles. `depends_on` order is valid.
- Jev (`plan-state.md`): coverage 1.64/2 before the input fix; the order holds (noul 0.90).

## Deviations

- **Host environment** (the same findings as #366's plan):
  - The runner is root, which ignores chmod. Five tests rely on chmod and fail identically on `origin/main`: core `publish.test.ts` "refuses the link", jev `session-start.test.ts` "refuses the link", and the ts/python/rust `read-failure.test.ts` "unreadable … file fails".
  - The ledger checks exclude them by name (`-t '^(?!.*(refuses the link|unreadable .* file fails)).*$'`).
  - The PATH-based golden tests need a deduplicated `PATH` (no `/bin`, `/sbin`), because this host has merged `/usr` and a dangling `grub-ntldr-img` link. Checks run with that `PATH`.
- **`bundles` check:** narrowed from the whole plugin test trees to the startup tests of every rebuilt startup bundle. Only startup bundles changed; post-tool bundles are byte-identical.
- **`report.ts` holds types and the writer only:** the strict Zod reader lives in the OpenCode bootstrap. A Zod schema in `@toolu/core/startup` pulled Zod into twelve small SessionStart bundles, about 3 100 lines each, on every host.
- **AC-6 order:** each plugin's dependencies are walked in sorted order as well as the names, so the order is canonical. The `delivery-flow` closure is `brainstorm, toolu, pr-babysit, toolu-review, delivery-flow`, and the spec was updated to match.
- **Report variable:** the adapter spells `TOOLU_STARTUP_REPORT` locally (`STARTUP_REPORT_VAR`), and a test pins it to core's `STARTUP_REPORT_ENV`. The npm-route smoke installs the published `@toolu/core` that the `^7.4.0` range resolves, 7.7.2 at the time. Importing the new export from there failed to link, and the host failed open. Plugin bundles are self-contained, so they carry the new writer either way.
- **Ledger hardening (review):** the ledger sits in the project, so helper paths outside the data root are never removed; a test covers it.
- **Empty `enabled` in `docs/opencode.md`:** the disable instructions are corrected. An empty selection is now ready with the core gates only (spec decision), so clearing `enabled` no longer reads as disabling enforcement.
