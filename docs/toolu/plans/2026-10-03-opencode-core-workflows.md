# OpenCode: review and core workflow skills on native tools — Plan

**Date:** 2026-10-03   **Status:** Approved   **Spec:** `docs/toolu/specs/2026-10-03-opencode-core-workflows-design.md`   **Topic:** Port toolu and toolu-review skills, agents, commands and session routing to OpenCode's native interfaces, with workflows proven on real isolated repositories (#358, OP-24)

## Evidence and approach

- **Inspected:**
  - `tools/toolu-opencode/scripts/lib/{rewrite,render,scan-surface,emit}.ts`;
  - the generated toolu and toolu-review tree;
  - `bundle-plugins.ts` and `tooling/src/pack-inventory.ts`;
  - `plugins/toolu/skills/setup/scripts/setup.ts` and its test;
  - `plugins/toolu/hooks/src/lifecycle/session-docs.ts` and `context-budget.ts`;
  - the pinned 1.18.34 binary's `task`, `skill` and `question` tool contracts;
  - the Jev port's hermetic and live harness (`jev-fixtures.ts`, `openSession`, `runHost`).
- **Approach (spec):**
  - a shared OpenCode column in the host mapping;
  - an exact-match OpenCode port table (`scripts/lib/opencode-port.ts`) applied to bodies and copied Markdown;
  - debug scripts staged into the npm package;
  - a host check in `setup.ts`;
  - an OpenCode model-routing session doc.
- **Recall:** the comemory search returned only an unrelated session summary.

## Workstream summary

port table + host mapping → generated surfaces and reference audit → package staging and debug helpers → setup refusal → session routing doc → hermetic workflow tests → docs → live host proof → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "port-table",
    "title": "OpenCode port table (exact-match, fail on drift) applied in renderMarkdown and planSkillResources; move toolu-review rewrites into it; agent tier rewrite; OpenCode column in host-mapping.md; regenerate",
    "ac_refs": [
      "AC-1",
      "AC-4",
      "AC-11"
    ],
    "paths": [
      "plugins/**",
      "docs/**",
      "README.md",
      "LICENSE",
      "tooling/conventions/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**"
    ],
    "input": "The real plugins/toolu and plugins/toolu-review sources through planSurface; a sandbox copy of plugins/toolu with one anchor removed and one duplicated; git diff against origin/main for every source surface except host-mapping.md and setup.ts",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts && bun run check:opencode-surface && git diff --quiet origin/main -- plugins/toolu/agents plugins/toolu/commands plugins/toolu/skills/commit plugins/toolu/skills/debug plugins/toolu/skills/deep-research plugins/toolu/skills/orchestrator plugins/toolu/skills/review-and-commit plugins/toolu/skills/setup/SKILL.md plugins/toolu-review/skills plugins/toolu/workflows/commit.md plugins/toolu/workflows/review-and-commit.md plugins/toolu/workflows/semantic-judgments.md",
    "model": "inherit"
  },
  {
    "id": "reference-audit",
    "title": "core-surfaces.test.ts: banned host tokens, skill/agent/path reachability, canonical host-valid frontmatter for all 14 core surfaces, host-mapping Claude/Codex cells unchanged vs origin/main",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4"
    ],
    "depends_on": [
      "port-table",
      "package"
    ],
    "paths": [
      "plugins/**",
      "docs/**",
      "README.md",
      "LICENSE",
      "tooling/conventions/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**",
      "tools/toolu-opencode/src/surfaces/**"
    ],
    "input": "The committed generated tree, stagePlugins output in a sandbox, git show origin/main:plugins/toolu/workflows/host-mapping.md",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/core-surfaces.test.ts",
    "model": "inherit"
  },
  {
    "id": "package",
    "title": "Stage plugins/toolu/scripts/debug-*.ts into @toolu/opencode; require them in the pack inventory",
    "ac_refs": [
      "AC-8"
    ],
    "paths": [
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tooling/src/pack-inventory.ts",
      "tooling/src/__tests__/npm-publish.test.ts",
      "plugins/toolu/scripts/**"
    ],
    "input": "bun pm pack --dry-run of tools/toolu-opencode after prepack staging",
    "check": "bun run test:pack",
    "model": "inherit"
  },
  {
    "id": "setup",
    "title": "setup.ts refuses on TOOLU_HOST_OVERRIDE=opencode (exit 2, capability message, no writes); Codex behavior unchanged",
    "ac_refs": [
      "AC-9",
      "AC-11"
    ],
    "paths": [
      "plugins/toolu/skills/setup/**",
      "plugins/toolu/skills/__tests__/setup-agents.test.ts"
    ],
    "input": "Sandbox HOME and CODEX_HOME; preview, install, remove --yes and an invalid verb with the override; the existing Codex cases without it",
    "check": "bun test --timeout 60000 plugins/toolu/skills/__tests__/setup-agents.test.ts",
    "model": "inherit"
  },
  {
    "id": "session-routing",
    "title": "model-routing-opencode.md rendered by session-docs on OpenCode; budgeted; Claude/Codex output unchanged",
    "ac_refs": [
      "AC-5",
      "AC-11"
    ],
    "paths": [
      "plugins/toolu/hooks/src/**",
      "plugins/toolu/hooks/docs/**",
      "plugins/toolu/hooks/dist/**",
      "plugins/toolu/scripts/context-budget.ts",
      "plugins/toolu/scripts/__tests__/context-budget.test.ts"
    ],
    "input": "The real toolu session-start bundle with TOOLU_HOST_OVERRIDE=opencode, codex and claude in sandbox projects; existing Claude and Codex session-start cases stay unmodified and green",
    "check": "bun test --timeout 60000 plugins/toolu/hooks/src/__tests__/model-routing.test.ts plugins/toolu/hooks/src/__tests__/session-start.test.ts && bun run test:context-budget && bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "workflows",
    "title": "core-workflows.test.ts via createTooluHooks: config hook registers 7+5+2 surfaces; review deny→write-state→allow with bare remote and findings keep deny; commit denied after failing test incl. --no-verify then allowed; debug helper on a real failing bun test for clone and staged npm layouts; setup refusal from the generated skill path",
    "ac_refs": [
      "AC-2",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9"
    ],
    "depends_on": [
      "port-table",
      "package",
      "setup"
    ],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "plugins/toolu/**",
      "plugins/toolu-review/**",
      "packages/toolu-core/src/**"
    ],
    "input": "Sandbox git repos with a local bare remote, package.json test scripts that fail then pass, a failing bun test file, toolu+toolu-review selected; the real published write-state.sh",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/core-workflows.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/opencode.md core workflows section; toolu and toolu-review READMEs OpenCode notes; regenerate resources",
    "ac_refs": [
      "AC-1",
      "AC-9"
    ],
    "depends_on": [
      "port-table",
      "setup",
      "session-routing"
    ],
    "paths": [
      "plugins/**",
      "docs/**",
      "README.md",
      "LICENSE",
      "tooling/conventions/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**"
    ],
    "input": "The edited docs, regenerated into generated/resources",
    "check": "rg -q 'toolu-quick-task' docs/opencode.md && rg -q 'OpenCode' plugins/toolu/README.md && rg -q '.opencode/tmp/push-review' plugins/toolu-review/README.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned host: skill tool loads toolu-review-review, push denied, write-state, push completes; toolu-debug helper names the failing test; task toolu-quick-task completes",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "workflows"
    ],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/core-workflows.live.test.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/toolu/**",
      "plugins/toolu-review/**"
    ],
    "input": "opencode-ai@1.18.34 in an isolated profile with the scripted loopback provider, a local bare remote and a failing bun test file",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 900000 tools/toolu-opencode/src/plugin/__tests__/core-workflows.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-9",
      "AC-11"
    ],
    "depends_on": [
      "port-table",
      "reference-audit",
      "package",
      "setup",
      "session-routing",
      "workflows",
      "docs"
    ],
    "paths": [
      "**"
    ],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- `tools/toolu-opencode/scripts/lib/opencode-port.ts` (new), `render.ts`, `rewrite.ts`, `scan-surface.ts`, `bundle-plugins.ts`
- `tools/toolu-opencode/scripts/__tests__/core-surfaces.test.ts` (new), `generate-surface.test.ts`
- `tools/toolu-opencode/src/plugin/__tests__/core-workflows.test.ts` (new), `core-workflows.live.test.ts` (new), `core-fixtures.ts` (new)
- `tools/toolu-opencode/generated/**` (regenerated)
- `plugins/toolu/workflows/host-mapping.md`
- `plugins/toolu/skills/setup/scripts/setup.ts`, `plugins/toolu/skills/__tests__/setup-agents.test.ts`
- `plugins/toolu/hooks/docs/model-routing-opencode.md` (new), `hooks/src/lifecycle/session-docs.ts`, `hooks/dist/session-start.js`, `plugins/toolu/scripts/context-budget.ts`
- `tooling/src/pack-inventory.ts`
- `docs/opencode.md`, `plugins/toolu/README.md`, `plugins/toolu-review/README.md`

## Verification

- Real generator output, the real staged package, real git repositories with bare remotes, the real published write-state helper, real `bun test` transcripts and the real `createTooluHooks`. No mocks.
- Boundary cases:
  - drifted or duplicated port anchors fail generation;
  - open review findings keep the push denied;
  - `--no-verify` cannot bypass a failing quality gate;
  - `setup.ts` refuses even an invalid verb on OpenCode;
  - Claude Code and Codex session routing output is byte-unchanged.
- Docs are synchronized and regenerated. The live host run is reported in the PR, separately from the hermetic suite.
- Delivery: scoped commits; `plan-ledger.js run <plan> --verify`; `toolu-review:review`; `verdict.js status` reporting `overall: ready`; push; PR to `main` starting `Closes Falconiere/toolu#358`; babysit handoff.

## Plan review

Jev checked step/requirement alignment. Ordering scored 0.37 → 0.72 once `depends_on` was supplied. Per-AC coverage is 0.60–0.89; the aggregate is lower because Jev sees check commands but not test bodies. Each step's `input` names its boundary inputs.

- port-table: 🟡 should-fix: its check ran `core-surfaces.test.ts`, which needs the package step's staged scripts. Fixed: the check is `generate-surface.test.ts`, and AC-3 moved to `reference-audit`.
- port-table, reference-audit, docs: 🟡 should-fix: `check:opencode-surface` reads every plugin source plus repository docs and README, and those paths were undeclared. Fixed: `plugins/**`, `docs/**`, `README.md`, `LICENSE` and `tooling/conventions/**` were added.
- AC-11: 🟡 should-fix: "source surfaces unchanged" had no mechanical check. Fixed: `git diff --quiet origin/main` over every source surface except `host-mapping.md` and `setup.ts`.

**Status:** Approved
