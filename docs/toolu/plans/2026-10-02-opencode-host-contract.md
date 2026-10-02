# OpenCode documented host contract (OP-01) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-host-contract-design.md   **Topic:** Pin `opencode-ai@1.18.34` and `@opencode-ai/plugin@1.18.34`, probe the real host, and check a 16-plugin capability matrix.

## Evidence and approach

**Recalled.** `ce014dd7` records the V2/docs mismatch. `d80b2ac0` records the facts verified on the pinned host during brainstorm: stdin must be closed, the loader is fail-open, `permission.ask` is never invoked, `before`-throw denies, and context hooks reach the provider.

**Inspected.**
- `tooling/src/opencode-capability-probe.ts`: the hardcoded V2 matrix that this work replaces.
- `tooling/src/check-portable-core-doc.ts`: the marker and citation pattern to follow.
- `@toolu/conformance/harness/{spawn,sandbox}`: `run` closes stdin, kills the process group on timeout and strips `OPENCODE_`/`TOOLU_` env; `createSandbox({ git: true })` provides a project and `HOME`.
- `tooling/guardrails.config.json` (closed `topLevel`), `knip.json` (adapter project is `src/**`), and root `tsconfig.json`/`format:check` (`tools/*/src` only).
- The `no-restricted-imports` bans on `@opencode/plugin` in the core, tooling and conformance `.oxlintrc.json` files.

**Approach.** Contract data and the typed probe plugins go in `tools/toolu-opencode/contract/` (unshipped; the SDK is a devDependency of the adapter). The harness goes in `tooling/src/opencode-host/`. The live CLI is `tooling/src/opencode-host-probe.ts`, and the hermetic checker is `tooling/src/opencode-host-contract.ts`. The checker joins `test:portable-core`.

## Workstream summary

Pin and SDK dependency → typed probe plugins → scripted provider and MCP fixture → live harness and committed evidence → matrix, checker and contract doc → portable-core migration, pointers and removal of the V2 probe → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "pin",
    "title": "Pin the SDK as an adapter devDependency; contract pin.json; tsconfig, knip, format and lint-ban wiring",
    "ac_refs": [
      "AC-1",
      "AC-6"
    ],
    "paths": [
      "tools/toolu-opencode/package.json",
      "tools/toolu-opencode/contract/pin.json",
      "bun.lock",
      "tsconfig.json",
      "knip.json",
      "package.json",
      "packages/toolu-core/.oxlintrc.json",
      "tooling/.oxlintrc.json",
      "tools/toolu-conformance/.oxlintrc.json"
    ],
    "input": "npm @opencode-ai/plugin@1.18.34 (integrity sha512-bC1EBL/QJ6LpC7liKZXr5JA+oa9qDAwTzpEgPxPsx1pbIUdhKOqg3/yoPEQgoa4YdO3hsgCWNFmbDE/DeOl/Ug==)",
    "check": "bun install --frozen-lockfile && test \"$(jq -r '.devDependencies[\"@opencode-ai/plugin\"]' tools/toolu-opencode/package.json)\" = \"$(jq -r .sdk.version tools/toolu-opencode/contract/pin.json)\" && bun run lint:ts"
  },
  {
    "id": "plugins",
    "title": "Typed probe plugins: probe (configurable hooks), init-throw, helper-export, module-default",
    "ac_refs": [
      "AC-1",
      "AC-3"
    ],
    "depends_on": [
      "pin"
    ],
    "paths": [
      "tools/toolu-opencode/contract/plugins/**",
      "tsconfig.json",
      "package.json"
    ],
    "input": "@opencode-ai/plugin@1.18.34 dist/index.d.ts (Plugin, Hooks, PluginModule)",
    "check": "bun run typecheck && bunx oxfmt --check tools/toolu-opencode/contract"
  },
  {
    "id": "fixtures",
    "title": "Scripted OpenAI-compatible SSE provider and stdio MCP fixture with real-transport tests",
    "ac_refs": [
      "AC-7"
    ],
    "paths": [
      "tooling/src/opencode-host/provider.ts",
      "tooling/src/opencode-host/mcp-server.ts",
      "tooling/src/opencode-host/__tests__/**",
      "tooling/guardrails.config.json"
    ],
    "input": "Real loopback HTTP requests shaped like @ai-sdk/openai-compatible chat completions; real `bun mcp-server.ts` stdio subprocess",
    "check": "bun test --timeout 60000 tooling/src/opencode-host/__tests__/provider.test.ts tooling/src/opencode-host/__tests__/mcp-server.test.ts && bun run guardrails"
  },
  {
    "id": "live",
    "title": "Live harness (install/resolve pinned CLI, warmed isolated profiles, scenarios, serve-based compaction) and committed probe-results.json",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3"
    ],
    "depends_on": [
      "plugins",
      "fixtures"
    ],
    "paths": [
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-host-probe.ts",
      "tools/toolu-opencode/contract/**",
      "package.json",
      "tooling/src/opencode-host/__tests__/results.test.ts",
      "tooling/src/opencode-host/__tests__/install.test.ts"
    ],
    "input": "opencode-ai@1.18.34 from npm (linux-x64), isolated HOME/XDG per scenario, scripted provider, MCP fixture; fake opencode executables printing a wrong version or exiting non-zero",
    "check": "bun run probe:opencode-host && bun test --timeout 60000 tooling/src/opencode-host/__tests__/results.test.ts tooling/src/opencode-host/__tests__/install.test.ts"
  },
  {
    "id": "checker",
    "title": "Capability matrix for all 16 plugins, declarations reader, hermetic contract checker and generated contract doc",
    "ac_refs": [
      "AC-4",
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "live"
    ],
    "paths": [
      "tools/toolu-opencode/contract/**",
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-host-contract.ts",
      "tooling/src/__tests__/opencode-host-contract.test.ts",
      "docs/opencode-host-contract.md",
      "plugins/*/hooks/hooks.json",
      "plugins/*/hooks/src/register.ts",
      "plugins/*/skills/**",
      "plugins/*/commands/**",
      "plugins/*/agents/**",
      "package.json",
      "plugins/*/.claude-plugin/plugin.json",
      "tools/toolu-opencode/package.json"
    ],
    "input": "Committed probe-results.json, real plugins/* manifests, installed pinned SDK declarations; sandbox copies mutated per failure case",
    "check": "bun run check:opencode-host && bun test --timeout 60000 tooling/src/__tests__/opencode-host-contract.test.ts"
  },
  {
    "id": "migrate-docs",
    "title": "Move portable-core to the documented contract, update its checker and tests, add pointers, AGENTS.md and testing.md rows, and delete the V2 capability probe",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "checker"
    ],
    "paths": [
      "docs/portable-core.md",
      "docs/opencode.md",
      "docs/conformance-report.md",
      "docs/testing.md",
      "tools/toolu-opencode/README.md",
      "AGENTS.md",
      "tooling/src/check-portable-core-doc.ts",
      "tooling/src/__tests__/check-portable-core-doc.test.ts",
      "tooling/src/opencode-capability-probe.ts",
      "tooling/src/__tests__/opencode-capability-probe.test.ts",
      "tooling/fixtures/portable-core/**",
      "package.json",
      "tooling/src/opencode-host/**",
      "tooling/src/opencode-host-contract.ts",
      "tools/toolu-opencode/contract/**",
      "docs/opencode-host-contract.md"
    ],
    "input": "Committed docs; sandbox doc copies containing opencode.ai/v2/ or missing the SDK pin",
    "check": "bun run test:portable-core && test ! -e tooling/src/opencode-capability-probe.ts && test ! -e tooling/fixtures/portable-core/capability-probe-results.json"
  },
  {
    "id": "gate",
    "title": "Full repository quality gate",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-4",
      "AC-5",
      "AC-6",
      "AC-7"
    ],
    "depends_on": [
      "migrate-docs"
    ],
    "paths": [
      "."
    ],
    "input": "Whole branch",
    "check": "bun run test"
  }
]
```

## Critical files

- **Create:**
  - `tools/toolu-opencode/contract/{pin.json,probe-results.json,capability-matrix.json}`
  - `tools/toolu-opencode/contract/plugins/{probe,init-throw,helper-export,module-default}.ts`
  - `tooling/src/opencode-host/{provider,mcp-server,install,profile,host-run,scenarios,results,matrix,declarations,doc-blocks,schema}.ts` and its `__tests__/`
  - `tooling/src/opencode-host-probe.ts`, `tooling/src/opencode-host-contract.ts`, `tooling/src/__tests__/opencode-host-contract.test.ts`
  - `docs/opencode-host-contract.md`
- **Modify:**
  - `tools/toolu-opencode/package.json`, `bun.lock`, `package.json`, `tsconfig.json`, `knip.json`, `tooling/guardrails.config.json`, and the three `.oxlintrc.json` files
  - `tooling/src/check-portable-core-doc.ts` and its test
  - `docs/portable-core.md`, `docs/opencode.md`, `docs/conformance-report.md`, `docs/testing.md`, `tools/toolu-opencode/README.md`, `AGENTS.md`
- **Delete:** `tooling/src/opencode-capability-probe.ts`, its test, and `tooling/fixtures/portable-core/capability-probe-results.json`

## Verification

- `live` compares every probe's `verdict` and `observed` with the committed `probe-results.json` and exits 1 on any drift. `results.test.ts` asserts the committed evidence directly:
  - AC-1: `load.local-file` and `load.config-file` are `supported`, with `optionsDelivered` false/true.
  - AC-2: each `deny.*` is `supported` with `sideEffect: false` and `errorReachedModel: true`; `permission.ask-hook` is `unsupported`; `permission.config-deny` and `permission.order` are `supported`; `context.system`, `context.prompt` and `context.compaction` are `supported`; `post.bash-exit` is `supported`; `post.tool-error` is `unsupported`.
  - AC-3: `load.init-throw` and `load.helper-export` are `unsupported`.

  A host change therefore fails the live comparison, and a hand-edited result fails `results.test.ts`. `install.test.ts` runs the real CLI with fake executables (wrong version, missing binary) and expects exit 1 without any install.

**Live.** `bun run probe:opencode-host` against the npm-installed pinned host reproduces every committed verdict. It covers AC-1 (both discovery routes), AC-2 (deny/permission/context/tool-failure) and AC-3 (fail-open loader). A wrong-version `TOOLU_OPENCODE_HOST_BIN` exits 1.

**Hermetic.** `bun run check:opencode-host` and its test mutate real copies and assert every named failure (AC-4, AC-5). `test:portable-core` rejects V2 citations (AC-6). The provider and MCP tests use real loopback HTTP and stdio (AC-7).

**Full gate.** `bun run test` passes with no warnings: format, lint, typecheck, guardrails, knip, jscpd, unit, pack, conformance, context budget, benchmarks, shell bench. Docs are synchronized in the `checker` and `migrate-docs` steps.

## Delivery

1. After the `gate` step, commit the scoped changes with a Conventional Commit: `feat(opencode): …`. Do not include `wip` subjects in the PR title.
2. Run `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run docs/toolu/plans/2026-10-02-opencode-host-contract.md --verify` (`TOOLU_PLUGIN_ROOT=plugins/toolu` in this checkout).
3. Run `toolu-review:review`, which writes version-2 push-review state with zero findings. Check that `bun plugins/toolu/hooks/dist/verdict.js status` reports `overall: ready`, then push `feat/335-opencode-verify-the-documented-host`. Rebase on `origin/main` first if it moved, and re-run the affected checks.
4. Create the PR against `main` with the title `feat(opencode): verify the documented host contract and inventory all 16 plugins`. The body starts with `Closes Falconiere/toolu#335` and `Part of Falconiere/toolu#334`, followed by a summary and the live-probe and `bun run test` evidence. Verify its number and head/base, report `pr-open`, then report `babysit` and invoke `pr-babysit:babysit`.

**Prerequisites** (already satisfied):

- An authenticated `gh api user`.
- A non-default branch.
- The `brainstorm`, `toolu`, `toolu-review` and `pr-babysit` skills installed.

## Review log

### Round 1 — Needs changes

Jev on step/AC alignment: AC-1 0.65, AC-2 0.76, AC-4 0.91, delivery readiness 0.02.

```
Delivery: 🔴 blocker: no final ledger verify, commit/push expectation or PR/babysit handoff. Add a Delivery section.
live: 🟡 should-fix: the check's link to AC-1/AC-2 verdicts is implicit and has no wrong-version boundary test. Add results.test.ts (direct assertions on the committed evidence) and install.test.ts (fake binaries), and state the drift semantics.
checker: 🟡 should-fix: paths omit plugins/*/.claude-plugin/plugin.json and the adapter package.json read by the check. Add them.
migrate-docs: 🟡 should-fix: test:portable-core runs check:opencode-host, but paths omit its inputs. Add them.
```

### Round 2 — Approved

Jev on revision 2: AC-1 0.85, AC-2 0.93, AC-4 0.91, delivery readiness 0.97.

- Every spec AC maps to at least one step (AC-1…AC-7, with `gate` covering all).
- Every step has a runnable check with real input and a boundary case.
- Paths cover what each check reads.
- Dependencies are acyclic: pin → plugins, fixtures → live → checker → migrate-docs → gate.
