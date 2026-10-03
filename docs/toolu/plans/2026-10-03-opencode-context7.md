# OpenCode context7 helper publication and documentation workflow (OP-14) — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-context7-design.md **Topic:** context7 owns its OpenCode startup instruction, runs its helper without `.env`, and ships a correct generated skill.

## Evidence and approach

A real-hook probe (`createTooluHooks`, temp project, `toolu` + `context7` selected) showed what works and what does not.

- Already working, from #342, #343 and #345:
  - `context7/search.sh` is a symlink under the data root.
  - `shell.env` exports `TOOLU_CONFIG_DIR` and `TOOLU_BUN`, and appends Bun's directory to PATH when PATH lacks Bun.
  - The generated `context7-context7` skill is discovered.
  - Startup order is alphabetical, so context7 publishes before toolu runs.
- Broken:
  - The only instruction is toolu's context7 line. That line is gated on Claude Code's `installed_plugins.json`, runs the helper through its shebang (so a project `.env` reaches it), and names no skill.
  - The generated skill labels the OpenCode path `# Claude Code` and keeps the Codex line.

Reused as they are: core `publishWrapper`, `renderHookOutput` and `sessionContext`; `createTooluHooks`; `shellEnvFor`/`applyShellEnv`; the conformance sandbox, `run` and `https-fixture`; the Jev suite's fixtures (`jev-fixtures.ts`: `binding`, `systemLines`, `bashEnv`, `inShell`); and the live harness in `tooling/src/opencode-host/` (as in `jev-delivery.live.test.ts`).

The design follows the approved spec. context7's SessionStart emits the instruction on OpenCode only, and toolu's block drops its context7 line on OpenCode only. A `context7-context7` rewrite fixes the generated skill.

## Workstream summary

context7 hook → toolu block → OpenCode delivery suite → generated skill → docs → live proof → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "c7-hook",
    "title": "context7 SessionStart on OpenCode: publishWrapper, one instruction naming '<bun>' --no-env-file '<path>' (or the user file alone) and context7-context7, silent on compact; Claude/Codex unchanged",
    "ac_refs": [
      "AC-1",
      "AC-3",
      "AC-5"
    ],
    "paths": [
      "plugins/context7/hooks/src/**",
      "plugins/context7/hooks/dist/**",
      "packages/toolu-core/src/startup/**",
      "packages/toolu-core/src/host/**",
      "tools/toolu-conformance/src/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "The real session-start bundle under TOOLU_HOST_OVERRIDE=opencode in a sandbox: startup and compact stdin, symlink and user-owned regular file, PATH without Bun (new opencode.test.ts); the existing Claude/Codex tests stay byte-identical to origin/main; a Codex-host case (CODEX_HOME sandbox) asserts silent stdout and the kept PATH advisory",
    "check": "bun test --timeout 60000 plugins/context7/hooks/src/__tests__/ && bun run check:plugin-bundles && git diff --quiet origin/main -- plugins/context7/hooks/src/__tests__/session-start.test.ts plugins/context7/hooks/src/__tests__/search.test.ts",
    "model": "inherit"
  },
  {
    "id": "toolu-block",
    "title": "toolu's mandate block omits the context7 line on OpenCode; Claude/Codex goldens unchanged",
    "ac_refs": [
      "AC-5",
      "AC-7"
    ],
    "paths": [
      "plugins/toolu/hooks/src/lifecycle/**",
      "plugins/toolu/hooks/src/session-start.ts",
      "plugins/toolu/hooks/src/__tests__/**",
      "plugins/toolu/hooks/dist/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "tooling/src/build-plugins.ts",
      "tooling/fixtures/**"
    ],
    "input": "toolu's lifecycle goldens (Claude, with the context7 line) unchanged",
    "check": "bun test --timeout 120000 plugins/toolu/hooks/src/__tests__/ && bun run check:plugin-bundles && git diff --quiet origin/main -- plugins/toolu/hooks/src/__tests__/fixtures/lifecycle-golden.json",
    "model": "inherit"
  },
  {
    "id": "delivery",
    "title": "OpenCode delivery suite: one context7 instruction per system call, none in compaction or when deselected; search/docs/429 through the instruction and skill commands with the shell.env env; .env ignored, env key sent; spaces and no Bun on PATH",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-3",
      "AC-7"
    ],
    "depends_on": [
      "c7-hook",
      "toolu-block",
      "skill"
    ],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/context7/hooks/dist/**",
      "plugins/toolu/hooks/dist/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "plugins/toolu/settings/**",
      "plugins/context7/.claude-plugin/**",
      "plugins/toolu/.claude-plugin/**"
    ],
    "input": "Temp OpenCode project (also one whose directory contains a space) selecting toolu+context7, and toolu only with a Claude installed_plugins.json listing context7; real createTooluHooks; HTTPS fixture for context7.com (search JSON, docs JSON, 429); /bin/sh with PATH /usr/bin:/bin; project .env with a ctx7sk_ secret",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/plugin/__tests__/context7-delivery.test.ts",
    "model": "inherit"
  },
  {
    "id": "skill",
    "title": "Generated context7-context7 skill: one # OpenCode command with \"$TOOLU_BUN\" --no-env-file and the data-root path; no Codex/Claude lines",
    "ac_refs": [
      "AC-4",
      "AC-2"
    ],
    "paths": [
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/context7/skills/**",
      "plugins/*/skills/**",
      "plugins/*/.claude-plugin/**",
      "plugins/*/README.md",
      "docs/**"
    ],
    "input": "The real plugins/context7/skills/context7/SKILL.md through the generator",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document context7 on OpenCode (data-root path, context7-context7, instruction, --no-env-file, compaction, selection)",
    "ac_refs": [
      "AC-1",
      "AC-4"
    ],
    "depends_on": [
      "skill"
    ],
    "paths": [
      "plugins/context7/README.md",
      "docs/context7/README.md",
      "docs/opencode.md",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/context7/skills/**",
      "plugins/*/skills/**",
      "plugins/*/README.md",
      "docs/**"
    ],
    "input": "The edited READMEs and docs/opencode.md, regenerated into generated/resources",
    "check": "rg -q 'context7-context7' plugins/context7/README.md && rg -q 'context7-context7' docs/context7/README.md && rg -q 'context7' docs/opencode.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned OpenCode host: instruction in the system request, context7-context7 in the request, scripted bash runs the instruction command against the HTTPS fixture without the .env secret",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "delivery",
      "skill"
    ],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/context7-delivery.live.test.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/context7/**",
      "tools/toolu-opencode/contract/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "plugins/toolu/**"
    ],
    "input": "opencode-ai@1.18.34 in an isolated profile with the scripted loopback provider and the context7.com HTTPS fixture; project .env with a ctx7sk_ secret",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 600000 tools/toolu-opencode/src/plugin/__tests__/context7-delivery.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "live-context7",
    "title": "One real context7.com search through the OpenCode-published command and shell.env environment, reported separately",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "delivery"
    ],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/context7-delivery.live.test.ts",
      "tools/toolu-opencode/src/**",
      "plugins/context7/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "plugins/toolu/**"
    ],
    "input": "Real context7.com, no key; one `search react` call",
    "check": "TOOLU_LIVE_CONTEXT7=1 bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/context7-delivery.live.test.ts",
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
      "AC-7"
    ],
    "depends_on": [
      "c7-hook",
      "toolu-block",
      "delivery",
      "skill",
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

- `plugins/context7/hooks/src/session-start.ts`, `hooks/src/context7/opencode.ts` (new), `hooks/dist/session-start.js`, `hooks/src/__tests__/opencode.test.ts` (new)
- `plugins/toolu/hooks/src/lifecycle/tool-mandates.ts`, `hooks/dist/session-start.js`
- `tools/toolu-opencode/src/plugin/__tests__/context7-delivery.test.ts` (new), `context7-delivery.live.test.ts` (new)
- `tools/toolu-opencode/scripts/lib/rewrite.ts`, `scripts/__tests__/generate-surface.test.ts`, `generated/**`
- `plugins/context7/README.md`, `docs/context7/README.md`, `docs/opencode.md`

## Verification

- Real bundles, real `createTooluHooks`, the real HTTPS fixture and real `/bin/sh`. No mocks.
- Failure and boundary cases:
  - a `.env` secret is never sent and never printed;
  - a 429 exits 22 with the stderr line;
  - PATH without Bun, and a project path containing a space;
  - a user-owned `search.sh` is named alone;
  - nothing appears in compaction or when context7 is deselected;
  - Claude and Codex output is unchanged.
- Docs are synchronized and regenerated. The live host run and the real context7.com call are reported in the PR, apart from the fixture evidence.
- Delivery: scoped commits; `plan-ledger.js run <plan> --verify`; `toolu-review:review`; `verdict.js status` reporting `overall: ready`; push; PR to `main` starting `Closes Falconiere/toolu#348`; babysit handoff.

## Plan review

AC coverage was checked mechanically by extracting the ledger with `sed` and `jq`. Every spec AC from AC-1 to AC-7 is referenced, no reference dangles, and the dependency graph is acyclic.

Jev then checked step and requirement alignment. Coverage moved from 0.40 to 0.37, paths from 0.36 to 0.35, and order from 0.74 to 0.58. Jev sees the check commands but not the test bodies, so these scores stay low. Each step's `input` names its boundary inputs. A follow-up choice named AC-6, the live evidence, as the weakest criterion (0.57).

- delivery: 🟡 should-fix: the step runs the generated skill's command (AC-2) but did not depend on `skill`. Fixed by adding `skill` to `depends_on`.
- c7-hook: 🟡 should-fix: the Codex half of AC-5 had no named input. Fixed with a Codex-host case.
- all steps: 🟡 should-fix: the checks read files outside the declared paths: core, conformance, `build-plugins.ts`, the contract pin, and the skills and docs read by `check:opencode-surface`. Fixed by adding those paths.
- live, live-context7: 🔵 consider: in `bun run test` these suites skip. Their ledger checks set `TOOLU_LIVE_OPENCODE=1` and `TOOLU_LIVE_CONTEXT7=1`, so they run. A missing host binary or a network failure makes them fail rather than skip. Their results are reported in the PR, apart from the fixture evidence.

**Status:** Approved
