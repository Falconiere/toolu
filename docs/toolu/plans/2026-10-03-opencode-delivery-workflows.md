# OpenCode: brainstorm and delivery-flow workflow semantics — Plan

**Date:** 2026-10-03   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-03-opencode-delivery-workflows-design.md   **Topic:** Port the generated brainstorm and delivery-flow skills and references to OpenCode's native tools, agents and paths, make the two core diagnostics OpenCode-aware, and prove the approval transitions in real isolated repositories (#355, OP-21)

## Evidence and approach

- **Inspected:**
  - `tools/toolu-opencode/scripts/lib/{opencode-port,rewrite,scan-surface}.ts`. `applyPort` runs on skill bodies before `rewriteBody`, and on every copied Markdown reference by its real path (`copiedMarkdown`).
  - The generated `brainstorm-brainstorm` and `delivery-flow-delivery-flow` trees, diffed against their sources.
  - `tools/toolu-opencode/src/host/runtime-env.ts`: `shell.env` sets `TOOLU_PLUGIN_ROOT` and `TOOLU_HOST_OVERRIDE=opencode`.
  - `packages/toolu-core/src/ledger/ledger-commands.ts` (preflight), `gates/plan-ledger.ts` (push gate) and `ledger/verdict*.ts`.
  - `plugins/toolu/hooks/src/lifecycle/session-notices.ts`.
  - #358's `core-surfaces.test.ts`, `core-workflows{,.live}.test.ts` and `core-fixtures.ts`.
  - The live harness, which ran green on this machine (pinned 1.18.34).
- **Approach (spec):**
  - extend `OPENCODE_PORTS` for the brainstorm and delivery-flow sources, reusing toolu's model-routing and semantic-judgments edit lists through shared constants;
  - give delivery-flow's `host-mapping.md` the OpenCode column;
  - make the preflight remedy and the migration notice OpenCode-aware;
  - regenerate.
- **Recall:** comemory returned the OpenCode leaf-startup and Bun `.env` notes, neither of which changes this plan.

## Workstream summary

port table + host mapping → surface audit → core diagnostics → hermetic workflow test → docs → live host proof → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "port",
    "title": "OPENCODE_PORTS entries for brainstorm SKILL.md and delivery-flow SKILL.md/execution.md/ledger.md/model-routing.md/semantic-judgments.md (shared constants with toolu's copies); OpenCode column in delivery-flow host-mapping.md; regenerate generated/",
    "ac_refs": ["AC-3", "AC-4", "AC-9"],
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
    "input": "The real plugins/brainstorm and plugins/delivery-flow sources through planSurface; git diff against the merge base for every plugin skill, agent and command except delivery-flow host-mapping.md",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts tools/toolu-opencode/scripts/__tests__/core-surfaces.test.ts && bun run check:opencode-surface && git diff --quiet $(git merge-base origin/main HEAD) -- 'plugins/*/skills' 'plugins/*/agents' 'plugins/*/commands' ':!plugins/delivery-flow/skills/delivery-flow/references/host-mapping.md' ':!plugins/*/skills/__tests__'",
    "model": "inherit"
  },
  {
    "id": "surface-audit",
    "title": "delivery-surfaces.test.ts: link closure of both skills resolves; banned host tokens; skill/agent ids and $TOOLU_PLUGIN_ROOT paths resolve against generated/ and a stagePlugins copy; frontmatter host-valid; tier->agent sentences; model-routing equals toolu's generated copy; host-mapping Claude/Codex cells equal origin/main; missing and duplicated anchor throw",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["port"],
    "paths": [
      "plugins/**",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "tools/toolu-opencode/src/inventory/**",
      "tools/toolu-opencode/src/surfaces/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "The committed generated tree; plugins staged by stagePlugins into a sandbox; a sandbox copy of plugins/delivery-flow with the ledger.md anchor removed and duplicated; git show origin/main:plugins/delivery-flow/skills/delivery-flow/references/host-mapping.md",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/delivery-surfaces.test.ts",
    "model": "inherit"
  },
  {
    "id": "core-diagnostics",
    "title": "plan-ledger preflight remedy names skill({ name: \"delivery-flow-delivery-flow\" }) on OpenCode only; deliveryFlowNotice OpenCode branch; rebuild plan-ledger.js and session-start.js; tests",
    "ac_refs": ["AC-7"],
    "paths": [
      "packages/**",
      "plugins/**",
      "tooling/src/build-plugins.ts",
      "tools/toolu-conformance/src/**"
    ],
    "input": "Real Draft/Approved plan and spec docs in a sandbox git repo, preflight run with TOOLU_HOST_OVERRIDE=opencode, claude, and unset; toolu session-start.js under opencode, claude and codex with the lifecycle goldens",
    "check": "bun test --timeout 60000 packages/toolu-core/src/ledger/__tests__/ledger.test.ts plugins/toolu/hooks/src/__tests__/session-start.test.ts plugins/toolu/hooks/src/__tests__/lifecycle-golden.test.ts && bun run check:plugin-bundles",
    "model": "inherit"
  },
  {
    "id": "workflows",
    "title": "delivery-workflows.test.ts: config hook registers both skills; preflight Draft spec/Draft plan/Approved transitions with the OpenCode remedy; ledger written under .opencode/tmp/plan-ledger; verdict blocked; push denied by plan-ledger gate with remote unchanged; after --verify and review write-state, verdict ready and push lands",
    "ac_refs": ["AC-1", "AC-6"],
    "depends_on": ["port", "core-diagnostics"],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/scripts/bundle-plugins.ts",
      "tools/toolu-opencode/generated/**",
      "packages/toolu-core/src/**",
      "plugins/**",
      "tools/toolu-conformance/src/**"
    ],
    "input": "A sandbox git repo with a bare remote, a branch changing a passing bun test file and README.md, Draft and Approved spec/plan docs under docs/toolu/, delivery-flow selected in .opencode/toolu/plugins.json and planLedger.mode block",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "docs/opencode.md Delivery workflows subsection; OpenCode notes in plugins/delivery-flow/README.md and plugins/brainstorm/README.md; regenerate",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["port", "core-diagnostics"],
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
    "input": "The edited docs and the regenerated tree",
    "check": "rg -q 'delivery-flow-delivery-flow' docs/opencode.md && rg -q 'toolu-implementer' docs/opencode.md && rg -q 'OpenCode' plugins/delivery-flow/README.md && rg -q 'OpenCode' plugins/brainstorm/README.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "delivery-workflows.live.test.ts on the pinned host: skill delivery-flow-delivery-flow, read references/spec.md, skill brainstorm-brainstorm, read references/design-questions.md, preflight Draft (error) and Approved, run --verify, push; remote has the commit",
    "ac_refs": ["AC-8"],
    "depends_on": ["workflows", "docs"],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/contract/**",
      "packages/toolu-core/src/**",
      "plugins/**"
    ],
    "input": "Pinned opencode-ai@1.18.34 with the scripted loopback provider, an isolated profile and the sandbox repo from the hermetic fixture",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 900000 tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": ["AC-9"],
    "depends_on": ["surface-audit", "workflows", "docs", "live"],
    "paths": ["**"],
    "input": "The whole tree",
    "check": "env -u npm_config_store_dir TMPDIR=/private/tmp bun run test",
    "model": "inherit"
  }
]
```

## Deviations

- Plan review: the `port` check first asserted all of `plugins/brainstorm` unchanged, which the `docs` step's README edit would break at `--verify`; narrowed to skills, agents and commands, extended to every plugin, and taken against the merge base so a moving `origin/main` cannot fail it. The `core-diagnostics` paths were widened to what `check:plugin-bundles` reads.

## Plan review

- port: 🔴 blocker (fixed): the source-unchanged check covered `plugins/brainstorm` whole, which the docs step's README edit breaks at `--verify`; now skills/agents/commands of every plugin, against the merge base.
- core-diagnostics: 🟡 should-fix (fixed): `check:plugin-bundles` reads every plugin and package source; paths widened.
- Spec AC-9: 🟡 should-fix (fixed in the spec): wording conflicted with the README docs step; now scoped to skills, agents, commands and the two runtime sources.
- AC coverage: `checkAcRefs` reports no dangling refs; AC-1…AC-9 each map to a step whose check exercises it. Jev: 0.53 → 0.37 before the fixes; gap localization afterwards favoured "none" (0.32, diffuse), confirmed by inspection.
- Status: Approved.

## Critical files

- `tools/toolu-opencode/scripts/lib/opencode-port.ts`
- `plugins/delivery-flow/skills/delivery-flow/references/host-mapping.md`
- `packages/toolu-core/src/ledger/ledger-commands.ts`, `packages/toolu-core/src/ledger/__tests__/ledger.test.ts`
- `plugins/toolu/hooks/src/lifecycle/session-notices.ts`, `plugins/toolu/hooks/src/__tests__/session-start.test.ts`
- `plugins/toolu/hooks/dist/plan-ledger.js`, `plugins/toolu/hooks/dist/session-start.js` (rebuilt)
- `tools/toolu-opencode/scripts/__tests__/delivery-surfaces.test.ts` (new)
- `tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.test.ts`, `delivery-workflows.live.test.ts`, `delivery-fixtures.ts` (new)
- `tools/toolu-opencode/generated/**` (regenerated)
- `docs/opencode.md`, `plugins/delivery-flow/README.md`, `plugins/brainstorm/README.md`

## Verification

- End to end: in a real repository, the generated skill's own commands move a Draft plan to Approved, record a ledger, and turn a denied push into one that reaches the remote. This runs hermetically through `createTooluHooks` and live on the pinned host.
- Failure and boundary cases: a Draft spec and a Draft plan are each refused with the OpenCode remedy; a push with an unverified ledger is denied and leaves the remote unchanged; a missing or duplicated port anchor fails generation; outside OpenCode the preflight text is unchanged.
- Docs: `docs/opencode.md` and both plugin READMEs, plus the regenerated `generated/resources/repo/docs/opencode.md`.
- Delivery: a scoped commit, then `plan-ledger.js run <plan> --verify`, then `toolu-review:review` with v2 state, then `verdict.js status` reporting `overall: ready`, then push, a PR to `main`, and the `pr-babysit:babysit` handoff. Prerequisites are `gh api user` and a non-default branch.
