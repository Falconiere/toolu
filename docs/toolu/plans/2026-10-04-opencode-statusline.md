# OpenCode statusline native diagnostics (OP-25) — Plan

**Date:** 2026-10-04 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-04-opencode-statusline-design.md **Topic:** Report toolu selection and readiness through the native status skill and structured host logs; scope the persistent statusline as Claude Code-only.

## Evidence and approach

Readiness exists only in the adapter process: `prepareEnforcement` in `tools/toolu-opencode/src/plugin/enforcement.ts` and `createTooluHooks` in `hooks.ts`. Diagnostics are sent through `hostLog` in `context.ts`, which today passes no `extra`. `shell.env` sets `TOOLU_CONFIG_DIR` to the data root and `TOOLU_PLUGIN_ROOT_<NAME>` for each selected plugin (`src/host/runtime-env.ts`). The startup ledger's atomic write (`src/bootstrap/ledger.ts` `writeLedger`) is the pattern for the record. The statusline `status.js` already supports `TOOLU_HOST_OVERRIDE=opencode` (`collect.ts`, `report.ts`). The generator's statusline rewrite and exclusion live in `tools/toolu-opencode/scripts/lib/rewrite.ts` and `constants.ts`. The capability matrix row is `tools/toolu-opencode/contract/capability-matrix.json` `statusline`. Live entry scenarios run through `tooling/src/opencode-entry-smoke.ts`, for example `scenarios-browser.ts`. The pinned SDK's `AppLogData.body.extra` accepts metadata.

Design, from the approved spec: core holds the shared record schema, the adapter writes it and logs one structured entry, statusline reads it on OpenCode only, the generated skill uses the `shell.env` paths, `ui` becomes `none` with a host-specific note, and docs are updated.

## Workstream summary

Core record schema → adapter writer and structured log → statusline report → generated surface and matrix → live host scenarios → docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "core-record",
    "title": "Add the shared OpenCode status record schema, path and strict reader to @toolu/core/startup",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["packages/toolu-core/src/startup/**", "packages/toolu-core/package.json"],
    "input": "Real files in a temp dir: a valid ready record, a valid not-ready record, a missing path, '{}', truncated JSON, a wrong version, an extra key, and a file larger than 256 KB",
    "check": "bun test packages/toolu-core/src/startup/__tests__/opencode-status.test.ts",
    "model": "inherit"
  },
  {
    "id": "adapter-record",
    "title": "Write the status record and send one structured toolu: status log entry for every settled startup",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["core-record"],
    "paths": ["tools/toolu-opencode/src/**", "tools/toolu-opencode/generated/**", "packages/toolu-core/src/**", "plugins/statusline/**", "plugins/jev/**", "plugins/toolu/**"],
    "input": "createTooluHooks with the real prepareEnforcement over the repo catalog in a sandbox git project selecting statusline and jev; a copied catalog missing jev's session-start bundle; an unwritable data root; a recording client; TOOLU_FAKE_SECRET=sk-test-359 in env",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/status-record.test.ts tools/toolu-opencode/src/plugin/__tests__/context.test.ts tools/toolu-opencode/src/plugin/__tests__/hooks.test.ts",
    "model": "inherit"
  },
  {
    "id": "statusline-report",
    "title": "Report toolu readiness, plugins, notes and the record in the OpenCode status output, leaving Claude and Codex unchanged; rebuild the bundles",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["core-record"],
    "paths": ["plugins/statusline/**", "packages/toolu-core/src/**", "tooling/src/build-plugins.ts"],
    "input": "The real status.js bundle spawned against a real git repo with ready, not-ready, missing, '{}' and truncated records under TOOLU_CONFIG_DIR, plus the existing Claude and Codex report fixtures; each run asserts stdout is exactly the report and stderr is empty",
    "check": "bun run check:plugin-bundles && bun test --timeout 60000 plugins/statusline/hooks/src/__tests__",
    "model": "inherit"
  },
  {
    "id": "surface-matrix",
    "title": "Generate the OpenCode status skill with shell.env paths, state the host-specific setup exclusion, set statusline.ui to none with the note, and regenerate the generated tree and contract doc",
    "ac_refs": ["AC-4"],
    "depends_on": ["statusline-report"],
    "paths": ["tools/toolu-opencode/**", "tooling/src/**", "docs/opencode-host-contract.md", "plugins/statusline/**"],
    "input": "The real generator over plugins/statusline, the committed generated/ tree, capability-matrix.json and probe-results.json",
    "check": "bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts tooling/src/__tests__/opencode-host-contract.test.ts && bun run check:opencode-surface && bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Prove on the pinned OpenCode host that the status skill reports the selected plugins as ready, logs a structured entry, keeps JSON stdout clean and writes no statusLine config",
    "ac_refs": ["AC-5", "AC-3"],
    "depends_on": ["adapter-record", "statusline-report", "surface-matrix"],
    "paths": ["tooling/src/**", "tools/toolu-opencode/**", "plugins/statusline/**", "plugins/jev/**", "packages/toolu-core/src/**", "package.json"],
    "input": "The packed @toolu/opencode with the exact opencode-ai 1.18.34 pin, in isolated profiles with a loopback scripted provider; a project selecting statusline and jev that loads the skill and runs its bash command; a project selecting only jev",
    "check": "bun run smoke:opencode-entry status.enabled status.disabled",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the OpenCode status skill, readiness lines, record, log entry and the Claude Code-only persistent statusline",
    "ac_refs": ["AC-4"],
    "depends_on": ["live"],
    "paths": ["plugins/statusline/README.md", "docs/statusline/README.md", "docs/opencode.md", "tools/toolu-opencode/README.md", "tools/toolu-opencode/generated/**"],
    "input": "Observed report text and log entry from the adapter, statusline and live runs",
    "check": "test \"$(grep -l 'Claude Code-only' plugins/statusline/README.md docs/statusline/README.md docs/opencode.md | wc -l | tr -d ' ')\" = 3 && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the complete repository gate on the final branch",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "The whole branch diff with focused and pinned-host checks green",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

Create `packages/toolu-core/src/startup/opencode-status.ts` and its test, re-exported from `packages/toolu-core/src/startup/startup.ts`. Create `tools/toolu-opencode/src/plugin/status-record.ts` and `__tests__/status-record.test.ts`. Modify `tools/toolu-opencode/src/plugin/context.ts` (log `extra`) and `hooks.ts` (write and log after settle). Modify `plugins/statusline/hooks/src/statusline/collect.ts` and `report.ts`, rebuild `plugins/statusline/hooks/dist/*.js`, and extend `__tests__/status.test.ts`. Update `tools/toolu-opencode/scripts/lib/rewrite.ts` and `constants.ts`, the generator test, `contract/capability-matrix.json`, `generated/**` and `docs/opencode-host-contract.md`. Add `tooling/src/opencode-host/scenarios-status.ts`, register it in `tooling/src/opencode-entry-smoke.ts`, and expose raw stdout from `host-run.ts`. Update the READMEs and `docs/opencode.md`.

## Verification

Use real files, real bundles, real git repos and the pinned host, with no mocks. Boundaries: missing, empty, malformed, oversized and wrong-version records; a not-ready startup; an unwritable data root; a fake secret in env; an unselected statusline. Show the tests can fail: while developing, revert the hooks.ts wiring (record not written) and confirm `status-record.test.ts` fails, then restore. Claude and Codex status output stays byte-identical, proven by the existing `status.test.ts` and `statusline.test.ts` expectations. Run focused tests, `check:plugin-bundles`, `check:opencode-surface`, `check:opencode-host`, the live smoke, `env -u npm_config_store_dir TMPDIR=/private/tmp bun run test`, the final ledger `run --verify`, the review and verdict readiness before push and PR. Rebase on `origin/main` if it moved before implementation and before pushing, then re-run the affected steps.

## Plan review

**Status:** Approved. Every spec AC (AC-1 to AC-5) maps to at least one step, with no dangling ids (checked by comparing the ledger's `ac_refs` with the spec's bold ids). Each step has a runnable check against real inputs, including boundary cases: missing, empty, malformed, oversized and wrong-version records; a not-ready startup; an unwritable data root; a fake secret; an unselected plugin. The order is acyclic: core-record → adapter-record and statusline-report → surface-matrix → live → docs → gate. Findings fixed during review: `adapter-record` now declares `tools/toolu-opencode/generated/**`, which `planSurfaces` reads, and `statusline-report` now asserts that stdout holds only the report and stderr is empty (AC-3). Jev judged the original step set 0.67 for full AC evidence and flagged the AC-3 stdout gap, which is now closed.
