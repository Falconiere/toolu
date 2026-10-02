# OpenCode core pre-tool enforcement (OP-04) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-core-pre-tool-design.md   **Topic:** Enforce selected core, registry, MCP and task rules in the pinned host before side effects.

## Evidence and approach

**Recalled:** `6990a41a` pins OpenCode 1.18.34 and confirms a `tool.execute.before` throw aborts calls. `ce014dd7` records the prior V2 mismatch. `2def75dc` records a real host task call whose supplied `model` reaches the hook.

**Inspected:** `tools/toolu-opencode/src/adapter/{evaluate,tool-before}.ts` already normalize native calls and use `dispatchPreTool`; `packages/toolu-core/src/dispatch/{dispatch,dispatch-walk}.ts` preserve module order, patch walks and decision precedence. `registry-run.ts` admits OpenCode registry entries when their presence is unknown. `mcpHook` and `agentTierHook` are standalone core entries. `prepareEnforcement` already has the resolved selected plugin manifests. `tooling/src/opencode-host/` and `smoke:opencode-entry` provide a real isolated host and scripted provider.

**Approach:** Add an optional selected-spec filter to the shared registry walk, used only by OpenCode. Route MCP to `mcpHook` and task to normal dispatch plus `agentTierHook`, preserving one call's identifiers and decision precedence. Prove denial before side effects in the pinned host, then synchronize docs and run the full gate.

## Workstream summary

Registry admission → OpenCode policy routes → real-host evidence → docs → full verification and delivery.

## Steps (machine-readable)

```json
[
  {
    "id": "registry",
    "title": "Restrict OpenCode pre-tools.d to selected plugin specs; assert the nine native names match BUILTIN_MODULES in order, one selected registry invocation, zero disabled invocations and unchanged existing-host traces",
    "ac_refs": ["AC-1", "AC-4"],
    "paths": [
      "packages/toolu-core/src/dispatch/**",
      "packages/toolu-core/src/registry/**",
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**",
      "plugins/toolu/hooks/src/pre-tools/**",
      "plugins/ast-grep/hooks/dist/search-nudge.js"
    ],
    "input": "Temp OpenCode registry with the committed ast-grep bundle and on-disk counter modules for a selected and a stale disabled spec; real Grep and protected Write requests; assert one selected counter line, zero disabled lines, no imports for an empty selected set, and deny short-circuit",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__ packages/toolu-core/src/dispatch/__tests__ packages/toolu-core/src/registry/__tests__/registry-run.test.ts packages/toolu-core/src/registry/__tests__/registry-register.test.ts packages/toolu-core/src/registry/__tests__/registry-paths.test.ts",
    "model": "inherit"
  },
  {
    "id": "routes",
    "title": "Run MCP through mcpHook and task through dispatch plus agentTierHook; assert model/effort and IDs, deny over ask/advisory, malformed fail-closed and unrelated skip",
    "ac_refs": ["AC-2", "AC-3", "AC-4"],
    "depends_on": ["registry"],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**",
      "packages/toolu-core/src/gates/**",
      "packages/toolu-core/src/dispatch/**",
      "plugins/toolu/settings/**"
    ],
    "input": "Real temp git project with protected .env, opencode.json MCP config, toolu config in block mode and a running plan ledger step; host-shaped task with wrong model and one without model; blocked MCP and malformed inputs; assert exact decisions and telemetry once",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__ tools/toolu-opencode/src/plugin/__tests__",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Add focused smoke on actual pinned OpenCode host: assert each denied tool state and unchanged side-effect bytes/markers for edit/write/patch, unsafe shell, commit with failing quality state, push without review state, MCP and task; assert allowed bash marker exists",
    "ac_refs": ["AC-2", "AC-3", "AC-4"],
    "depends_on": ["routes"],
    "paths": [
      "tooling/src/opencode-pretool-smoke.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/contract/**",
      "plugins/toolu/**",
      "package.json"
    ],
    "input": "Pinned opencode-ai@1.18.34 in isolated profile/project with native permissions set to allow, actual toolu package entry, scripted loopback provider, real git repo and local MCP server; task.model mismatch against a running ledger step, with child-session log and marker checked absent",
    "check": "bun run smoke:opencode-pretool && bun run smoke:opencode-entry",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the exact pinned pre-tool routes and selection semantics in OpenCode and portable-core docs",
    "depends_on": ["live"],
    "paths": [
      "docs/opencode.md",
      "docs/portable-core.md",
      "docs/opencode-host-contract.md",
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/plugin/**"
    ],
    "input": "Implemented routes and observed smoke results",
    "check": "bun run test:portable-core && bun run check:opencode-host",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run full repository gate, existing-host conformance and pinned package checks on the final branch",
    "ac_refs": ["AC-5"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "Whole branch after implementation, smoke and documentation",
    "check": "bun run test:conventions",
    "model": "inherit"
  }
]
```

## Critical files

- Modify `packages/toolu-core/src/dispatch/{dispatch,dispatch-walk}.ts`, `packages/toolu-core/src/registry/registry-run.ts`, `tools/toolu-opencode/src/adapter/{evaluate,tool-before}.ts`, `tools/toolu-opencode/src/plugin/enforcement.ts`, root `package.json`, `docs/opencode.md`, and `docs/portable-core.md`.
- Extend colocated tests in `tools/toolu-opencode/src/adapter/__tests__/` and `packages/toolu-core/src/{dispatch,registry}/__tests__/` as needed. Create `tooling/src/opencode-pretool-smoke.ts` and a focused scenario under `tooling/src/opencode-host/`.
- Read unchanged: pinned host contract/probe data, `plugins/toolu/hooks/src/pre-tools/builtins.ts`, the core standalone MCP/task entries and the existing live-host harness.

## Verification

- **End to end:** The actual host receives scripted tool calls under native `allow` permissions. For protected write/edit/patch, check original bytes and every patch destination. For unsafe bash, commit against a failing quality state, and push with missing review state in `block` mode, check the command's leading marker did not run. For MCP, check the server's tool marker is absent. For a task-model mismatch, check no child session/tool entry and no child marker. Every denied tool state must name its matching toolu gate; a benign bash marker must appear.
- **Boundary:** A stale disabled registry file remains inert, a selected counter runs once, malformed known tools fail closed, an unknown tool retains native behavior, and a task without a model inherits its tier.
- **Existing hosts:** The optional registry filter is absent for Claude/Codex; dispatcher golden traces and `bun run test:conformance` remain equivalent.
- **Docs:** The OpenCode and portable-core descriptions match the tested routes; the broader migration guide remains with #363.
- **Delivery:** After focused checks and `bun run test`, commit scoped changes, run final ledger `--verify`, run `toolu-review:review` with v2 zero-finding state, confirm `verdict.js status` is ready, push, open a PR to `main` with the required issue lines, then hand off to `pr-babysit:babysit`. Fetch and rebase before implementation and before the final push if `origin/main` moved.

## Deviations

- The orchestrator reproduced `registry-import-cost.test.ts` timing out on clean `origin/main` under this shared machine's load (30–44 on 8 CPUs) and directed us to stop local retries. The registry and unit checks exclude only that file locally. Run every other `test:ts` stage individually, including typecheck, lint, functional suites, host contract, conformance and deterministic checks. Record the clean-main reproduction in the PR; CI's isolated `typescript` job must pass the complete unmodified `bun run test` before merge. The ledger's final check is `test:conventions`; the remaining stage outcomes are recorded in the PR.

## Plan review

- Live step: 🟡 should-fix (resolved): the draft named tool classes but left the marker checks implicit. The step and verification now specify native allow, exact denied states, unchanged patch destinations, a leading shell marker, MCP server marker, and absent child session/marker.
- Registry and routes steps: 🟡 should-fix (resolved): the draft described checks broadly. The steps now name ordered native identities, exact selected/disabled counts, task IDs/model/effort, telemetry once, deny priority, and malformed/unknown boundaries.
- Dependency order: registry → routes → live → docs → gate; every AC has a direct step reference and check. The full gate and final delivery preflight remain explicit.
- **Status:** Approved.
