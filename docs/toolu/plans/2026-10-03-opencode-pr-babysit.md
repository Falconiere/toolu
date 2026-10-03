# OpenCode pr-babysit fixer dispatch and lifecycle (OP-23) — Plan

**Date:** 2026-10-03   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-03-opencode-pr-babysit-design.md   **Topic:** OpenCode controller loop, `--host opencode` routing, `opencode run` fixer subprocess with scoped denies, native worktree, packaging and generated text.

## Evidence and approach

Inspected on `origin/main` at `63ae77fd`:

- `plugins/pr-babysit/hooks/src/babysit/fixer-route.ts`: `Host` is claude/codex/cursor; `routeFix` rejects other `--host`; `configPaths` knows Claude and Codex roots; Jev from `~/.claude` or `~/.codex`.
- `plugins/pr-babysit/hooks/src/babysit/fixer-dispatch.ts`: herdr-only `launch`, `wait`, `settleGroup`, `stopAgent`, `worktree`, `cleanup`; `dirt()` ignores `.claude|.codex|.cursor`; reads `skills/babysit/references/fixer-brief.md` from the plugin root.
- `tools/toolu-opencode/scripts/bundle-plugins.ts` stages `hooks/dist/*.js` only; `tooling/src/pack-inventory.ts` requires `committedBundles`.
- `tools/toolu-opencode/scripts/lib/opencode-port.ts` (#358) is the exact-anchor port table; `generated/skills/pr-babysit-babysit-73c340c6/SKILL.md` is Codex text.
- Live harness: `tooling/src/opencode-host/` (`install`, `runHost`, scripted provider keyed by `PROBE:<id>`), registered in `tooling/src/opencode-entry-smoke.ts`.
- Pinned CLI help (1.18.34): `opencode run --format json --dir --agent --model --variant --auto`; binary honors `OPENCODE_CONFIG_CONTENT` (config merge) and `OPENCODE_PERMISSION`.
- Tick accepts `--snapshot-in` (captured `snapshots/toolu-165.json`).

Approach as in the spec: native controller text in the shared workflow, port edits for the OpenCode copy, `opencode` host in routing, a subprocess transport for `opencode` groups with an `OPENCODE_CONFIG_CONTENT` agent carrying the denies, a native worktree for all-`opencode` plans, staged `fixer-brief.md`, and three live scenarios.

## Workstream summary

routing → subprocess transport + native worktree → packaging → shared workflow/helper/docs → generated OpenCode text → live harness + scenarios → live run → capability matrix note → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "route",
    "title": "fixer-route: opencode host kind/CLI, --host opencode, OpenCode config files via @toolu/core/config, routing.opencode default {} rows, Jev from $TOOLU_CONFIG_DIR/jev/jev.sh with --no-env-file on OpenCode; Claude/Codex output unchanged",
    "ac_refs": ["AC-4"],
    "paths": [
      "plugins/pr-babysit/hooks/src/**",
      "plugins/pr-babysit/hooks/dist/**",
      "plugins/pr-babysit/scripts/__tests__/fixtures/**",
      "packages/toolu-core/src/config/**",
      "packages/toolu-core/src/host/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "Captured review-items.json and jev/fix-tiers.json fixtures; sandbox TOOLU_USER_CONFIG_DIR and .opencode/toolu.config.json files; a PATH with and without an opencode executable; a jev.sh under the sandbox data root",
    "check": "bun test --timeout 60000 plugins/pr-babysit/hooks/src/__tests__/fixer-native.test.ts && bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "transport",
    "title": "fixer-dispatch: opencode groups run as detached `opencode run --agent pr-babysit-fixer` subprocesses with OPENCODE_CONFIG_CONTENT denies, log file, pid+start identity, group polling in wait, TERM/KILL stop; native git worktree for all-opencode plans; herdr probed only when a pane is needed; .opencode artifacts are not fixer work",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["route"],
    "paths": [
      "plugins/pr-babysit/hooks/src/**",
      "plugins/pr-babysit/hooks/dist/**",
      "plugins/pr-babysit/skills/babysit/references/fixer-brief.md",
      "plugins/pr-babysit/scripts/__tests__/fixtures/**",
      "packages/toolu-core/src/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "Real sh processes with a backgrounded grandchild (sleep) for group liveness and stop; a reused-pid identity mismatch; caller OPENCODE_CONFIG_CONTENT values (bash string, bash object, invalid JSON); a real (non-dry) start of an all-opencode plan in a sandbox git repo with a local bare origin and a PATH without opencode (native worktree created, group failed/agent_start_failed, actions ledger unchanged), then cleanup removing it, and cleanup refusing a worktree with an uncommitted file (worktree_dirty); fixerOutcome on provider-limit log text (host_limited) and plain text (no_report); a dry-run start with an all-opencode plan; state copied from states/toolu-165-initial.json; existing herdr dry-run tests unchanged",
    "check": "bun test --timeout 60000 plugins/pr-babysit/hooks/src/__tests__/ && bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "package",
    "title": "Stage plugins/pr-babysit/skills/babysit/references/fixer-brief.md in @toolu/opencode and require it in the pack inventory",
    "ac_refs": ["AC-6"],
    "paths": [
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tooling/src/pack-inventory.ts",
      "tooling/src/__tests__/**",
      "tools/toolu-opencode/package.json",
      "plugins/**"
    ],
    "input": "bun pm pack --dry-run of tools/toolu-opencode",
    "check": "bun run test:pack",
    "model": "inherit"
  },
  {
    "id": "workflow",
    "title": "Shared workflow gains OpenCode start/resume, cancel, state path, inline task column and subprocess fixer text; helper.md, README, docs/config.md, docs/opencode.md and docs/pr-babysit describe the OpenCode host",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["transport"],
    "paths": [
      "plugins/pr-babysit/workflows/babysit.md",
      "plugins/pr-babysit/skills/**",
      "plugins/pr-babysit/README.md",
      "plugins/pr-babysit/commands/**",
      "docs/config.md",
      "docs/opencode.md",
      "docs/pr-babysit/**",
      "plugins/toolu/scripts/**",
      "tooling/src/**"
    ],
    "input": "The edited Markdown sources",
    "check": "bun run test:context-budget && bun run guardrails && bun test --timeout 60000 tooling/src/__tests__/conventions-guardrails.test.ts",
    "model": "inherit"
  },
  {
    "id": "generated",
    "title": "Port table edits for pr-babysit SKILL.md, references/helper.md and workflows/babysit.md; regenerate; audit test: OpenCode strings present, Claude/Codex controller strings absent, links resolve, handoff section kept",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["workflow"],
    "paths": [
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/**",
      "plugins/*/skills/**",
      "plugins/*/workflows/**",
      "plugins/*/commands/**",
      "plugins/*/agents/**",
      "plugins/*/.claude-plugin/**",
      "plugins/*/README.md",
      "docs/**",
      "README.md",
      "LICENSE"
    ],
    "input": "The real pr-babysit sources through the generator",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/ && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "harness",
    "title": "Scripted provider '*' script for nested fixer sessions; scenarios-babysit.ts (babysit.fixer, babysit.no-report, babysit.cancel) registered in opencode-entry-smoke; selection test updated",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-6", "AC-7"],
    "depends_on": ["transport", "package", "generated"],
    "paths": [
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-entry-smoke.ts",
      "tooling/src/__tests__/opencode-entry-smoke.test.ts"
    ],
    "input": "Scenario ids and the provider's script selection",
    "check": "bun test --timeout 60000 tooling/src/__tests__/opencode-entry-smoke.test.ts tooling/src/opencode-host/__tests__/",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned-host proof: an OpenCode controller runs tick/route/start/wait/record through its bash; the OpenCode fixer fixes a seeded failing test, push/gh/task denied, reports done; no-report and cancel settle accurately",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-6", "AC-7"],
    "depends_on": ["harness"],
    "paths": [
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-entry-smoke.ts",
      "tools/toolu-opencode/**",
      "plugins/**",
      "packages/toolu-core/src/**"
    ],
    "input": "Pinned opencode-ai@1.18.34, scripted loopback provider, isolated profile; sandbox git repo with src/sum.ts bug and failing src/sum.test.ts, local bare origin; captured snapshots/toolu-165.json",
    "check": "bun run smoke:opencode-entry babysit.fixer babysit.no-report babysit.cancel",
    "model": "inherit"
  },
  {
    "id": "matrix",
    "title": "Capability-matrix pr-babysit tick-scheduling note states the OpenCode controller loop; host-contract doc block regenerated",
    "ac_refs": ["AC-5"],
    "depends_on": ["live"],
    "paths": [
      "tools/toolu-opencode/contract/**",
      "docs/opencode-host-contract.md",
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-host-contract.ts",
      "plugins/*/.claude-plugin/**",
      "plugins/*/.codex-plugin/**",
      "plugins/*/hooks/hooks.json"
    ],
    "input": "The committed matrix, probe results and manifests",
    "check": "bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["route", "transport", "package", "workflow", "generated", "harness", "matrix"],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Plan review (2026-10-03)

- transport: 🟡 should-fix: spec failure modes (spawn failure, dirty cancel, host_limited) lacked checks. Fixed: real sandbox start/cleanup and fixerOutcome inputs.
- Verification: 🟡 should-fix: no delivery sequence (Jev Noul 0.05). Fixed: Delivery section.
- AC coverage: AC-1…AC-7 each mapped (route AC-4; transport AC-2/3; package AC-6; workflow, generated AC-5/7; harness, live AC-1/2/3/6/7; matrix AC-5; gate all). Jev alignment 1.23/2 before the transport fix.

## Deviations

- **route:** OpenCode config paths are resolved in `fixer-route.ts` instead of importing `@toolu/core/config` (its `configFiles` is not exported and the module pulls zod into the bundle); `fixer-route-opencode.test.ts` asserts equality with core's `configFiles`. `commandAvailable` passes the current `PATH` to `Bun.which`, which otherwise reads the PATH the process started with.
- **transport:** `stopGroup` also ends descendants outside the fixer's process group: on the pinned host each bash tool call runs in its own group (found by `babysit.cancel`). An agent-level `task: deny` removes `task` from the fixer's tools, so the scenario checks that it is not offered.
- **generated:** the port table gained a `cut` edit (anchor to anchor, or to the end) for whole host sections, in `opencode-port-pr-babysit.ts`. AC-5's ban list names the Claude state-path instructions (`/tmp/pr-babysit-${SLOT}`, `/tmp/pr-babysit-<slot>`); captured example output in `helper.md` keeps its real `/tmp` paths.
- **live:** the scenario puts the pinned binary first on `PATH`; the dispatcher starts whatever `opencode` PATH names.

## Critical files

- `plugins/pr-babysit/hooks/src/babysit/fixer-route.ts`, `babysit/fixer-dispatch.ts`, `babysit-route-fix.ts`, `hooks/dist/*.js`
- `plugins/pr-babysit/hooks/src/__tests__/fixer-native.test.ts` (+ a new `fixer-opencode.test.ts` for process groups and config content)
- `plugins/pr-babysit/workflows/babysit.md`, `skills/babysit/SKILL.md` (only through the port), `skills/babysit/references/helper.md`, `README.md`
- `tools/toolu-opencode/scripts/lib/opencode-port.ts`, `scripts/bundle-plugins.ts`, `scripts/__tests__/pr-babysit-surfaces.test.ts`, `generated/**`
- `tooling/src/pack-inventory.ts`, `tooling/src/opencode-host/provider.ts`, `scenarios-babysit.ts`, `tooling/src/opencode-entry-smoke.ts`, `tooling/src/__tests__/opencode-entry-smoke.test.ts`
- `docs/config.md`, `docs/opencode.md`, `docs/pr-babysit/README.md`, `tools/toolu-opencode/contract/capability-matrix.json`, `docs/opencode-host-contract.md`

## Verification

- End to end: `bun run smoke:opencode-entry babysit.fixer babysit.no-report babysit.cancel` on the pinned host: pass lines recorded in the PR.
- Boundaries: invalid caller `OPENCODE_CONFIG_CONTENT`, missing `opencode` CLI, pid reuse, grandchild survival, dirty worktree on cancel, mixed plan without herdr.
- Existing hosts: the current fixer-native, write-side, record and tick tests keep passing unchanged; Claude/Codex routing outputs equal.
- Docs: workflow, helper, README, config and OpenCode docs updated in `workflow`; regenerated `generated/` checked by `check:opencode-surface`.
- `bun run test` (with `TMPDIR=/private/tmp`, `npm_config_store_dir` unset locally).

## Delivery

Scoped `feat(pr-babysit)` commit → `bun plugins/toolu/hooks/dist/plan-ledger.js run docs/toolu/plans/2026-10-03-opencode-pr-babysit.md --verify` → `toolu-review:review` with version 2 state → `verdict.js status` reports `overall: ready` → rebase on `origin/main` if it moved and re-run affected checks → push `feat/357-opencode-port-pr-babysit-native` → PR to `main` (`Closes Falconiere/toolu#357`, `Part of Falconiere/toolu#334`) → `report pr-open` → `report babysit` → `/pr-babysit:babysit`. Prerequisites checked first: `gh api user`, non-default branch, installed brainstorm/toolu/toolu-review/pr-babysit skills.
