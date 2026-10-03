# OpenCode Jira startup and issue workflow (OP-17) — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-jira-design.md **Topic:** jira owns its OpenCode startup instruction, runs its helper and plan checks without `.env`, keeps the write boundary explicit, and ships a correct generated skill.

## Evidence and approach

Inspected on `origin/main` at `6533a205`:

- Already working, from #342, #343 and #345: `jira/jira.sh` is published as a symlink under the data root; `shell.env` exports `TOOLU_CONFIG_DIR`, `TOOLU_BUN`, `TOOLU_HOST_OVERRIDE=opencode` and `TOOLU_PROJECT_CONFIG_DIRNAME=.opencode` (so plan files land under `.opencode/tmp`); the generated `jira-jira` skill is discovered.
- Broken: jira's SessionStart is silent; toolu's prompt hint names a `jira` skill that does not exist on OpenCode and fires without jira selected; the generated skill shows Codex and mislabelled Claude lines and `.claude`/`.codex` plan paths; the helper and the plan checks' `"$JIRA"` run through `#!/usr/bin/env bun`, which loads the project `.env` (Bun 1.4.2 verified; `BUN_OPTIONS=--no-env-file` turns it off for shebang scripts, also verified).
- Precedent: context7 (#381) — `context7/opencode.ts`, `context7-delivery.test.ts`, `context7-delivery.live.test.ts`, the `opencodeContext7` rewrite.

Reused as they are: core `publishWrapper`, `renderHookOutput`, `sessionContext`; `createTooluHooks`; `shellEnvFor`/`applyShellEnv`; the jira harness (`startJira`, recorded `fixtures/*.json`); `@toolu/conformance/https-fixture`; `jev-fixtures.ts` (`binding`, `systemLines`, `bashEnv`, `inShell`, `hook`); the live harness in `tooling/src/opencode-host/`.

## Workstream summary

jira hook + plan children → toolu prompt hint → generated skill + source skill sentence → OpenCode delivery suite → docs → live proof → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "jira-hook",
    "title": "jira SessionStart on OpenCode: publishWrapper, one instruction naming '<bun>' --no-env-file '<path>' (or the user file alone) and jira-jira with the write boundary, silent on compact; plan probe/check children get --no-env-file in BUN_OPTIONS on OpenCode; Claude/Codex unchanged",
    "ac_refs": ["AC-1", "AC-4", "AC-5", "AC-8"],
    "paths": [
      "plugins/jira/hooks/src/**",
      "plugins/jira/hooks/dist/**",
      "packages/toolu-core/src/startup/**",
      "packages/toolu-core/src/host/**",
      "packages/toolu-core/src/rest/**",
      "packages/toolu-core/src/cli/**",
      "tools/toolu-conformance/src/**",
      "tooling/src/build-plugins.ts"
    ],
    "input": "The real session-start bundle under TOOLU_HOST_OVERRIDE=opencode in a sandbox (startup and compact stdin, symlink and user-owned file, data root with a space and a quote, run in /bin/sh with PATH /usr/bin:/bin); the real jira bundle running plan run with a check `\"$JIRA\" user whoami` against the HTTPS fixture, env JIRA_EMAIL+JIRA_API_TOKEN and a project .env JIRA_PAT secret, with and without the override; the existing jira tests unchanged",
    "check": "bun test --timeout 60000 plugins/jira/hooks/src/__tests__/ && bun run check:plugin-bundles && git diff --quiet origin/main -- plugins/jira/hooks/src/__tests__/session-start.test.ts plugins/jira/hooks/src/__tests__/plan-run.test.ts",
    "model": "inherit"
  },
  {
    "id": "toolu-hint",
    "title": "toolu's prompt hint omits its jira line on OpenCode; Claude/Codex goldens unchanged",
    "ac_refs": ["AC-6", "AC-8"],
    "paths": [
      "plugins/toolu/hooks/src/lifecycle/**",
      "plugins/toolu/hooks/src/user-prompt-submit.ts",
      "plugins/toolu/hooks/src/__tests__/**",
      "plugins/toolu/hooks/dist/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "tooling/src/build-plugins.ts",
      "tooling/fixtures/**"
    ],
    "input": "toolu's lifecycle goldens (Claude, with the Jira hint) unchanged",
    "check": "bun test --timeout 120000 plugins/toolu/hooks/src/__tests__/ && bun run check:plugin-bundles && git diff --quiet origin/main -- plugins/toolu/hooks/src/__tests__/fixtures/lifecycle-golden.json plugins/toolu/hooks/src/__tests__/user-prompt-submit-cases.ts",
    "model": "inherit"
  },
  {
    "id": "skill",
    "title": "Generated jira-jira skill: one # OpenCode command with \"$TOOLU_BUN\" --no-env-file and the data-root path, .opencode plan paths, no Codex/Claude lines; source skill states loading never authorizes a write",
    "ac_refs": ["AC-7", "AC-5", "AC-2"],
    "paths": [
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jira/skills/**",
      "plugins/*/skills/**",
      "plugins/*/.claude-plugin/**",
      "plugins/*/README.md",
      "docs/**"
    ],
    "input": "The real plugins/jira/skills/jira/SKILL.md through the generator",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "delivery",
    "title": "OpenCode delivery suite: one jira instruction per system call, none in compaction, none and no prompt hint when deselected; zero requests from loading; read, mutation, 401/400 and no-credential runs through the instruction and skill commands with the shell.env env; jira-cli config + .env host override ignored; no secret in output",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-5", "AC-6"],
    "depends_on": ["jira-hook", "toolu-hint", "skill"],
    "paths": [
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jira/hooks/dist/**",
      "plugins/jira/hooks/src/__tests__/fixtures/**",
      "plugins/toolu/hooks/dist/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "plugins/toolu/settings/**",
      "plugins/jira/.claude-plugin/**",
      "plugins/toolu/.claude-plugin/**"
    ],
    "input": "Temp OpenCode project (also under a directory with a space) selecting toolu+jira, and toolu only; real createTooluHooks; HTTPS fixture for acme.atlassian.net and media.example.net with the recorded issue.json; /bin/sh with PATH /usr/bin:/bin; a jira-cli config file with a token; a project .env overriding JIRA_BASE_URL",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/plugin/__tests__/jira-delivery.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document jira on OpenCode (data-root path, jira-jira, instruction, --no-env-file, nested checks, .opencode plan paths, compaction, selection, toolu hint omitted)",
    "ac_refs": ["AC-1", "AC-7"],
    "depends_on": ["skill"],
    "paths": [
      "plugins/jira/README.md",
      "docs/jira/README.md",
      "docs/opencode.md",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jira/skills/**",
      "plugins/*/skills/**",
      "plugins/*/README.md",
      "docs/**"
    ],
    "input": "The edited READMEs and docs/opencode.md, regenerated into generated/resources",
    "check": "rg -q 'jira-jira' plugins/jira/README.md && rg -q 'jira-jira' docs/jira/README.md && rg -q 'jira' docs/opencode.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned OpenCode host: instruction and jira-jira in the system request; scripted skill load then bash issue get against the HTTPS fixture; one GET, no write, no .env secret",
    "ac_refs": ["AC-9"],
    "depends_on": ["delivery", "skill"],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/jira-delivery.live.test.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jira/**",
      "tools/toolu-opencode/contract/**",
      "packages/toolu-core/src/**",
      "tools/toolu-conformance/src/**",
      "plugins/toolu/**"
    ],
    "input": "opencode-ai pinned in tools/toolu-opencode/contract, isolated profile, scripted loopback provider, acme.atlassian.net HTTPS fixture; project .env with a JIRA_PAT secret",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 600000 tools/toolu-opencode/src/plugin/__tests__/jira-delivery.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8"],
    "depends_on": ["jira-hook", "toolu-hint", "skill", "delivery", "docs"],
    "paths": ["**"],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- `plugins/jira/hooks/src/session-start.ts`, `hooks/src/jira/opencode.ts` (new), `hooks/src/jira/plan-run.ts`, `hooks/dist/session-start.js`, `hooks/dist/jira.js`, `hooks/src/__tests__/opencode.test.ts` (new)
- `plugins/jira/skills/jira/SKILL.md`
- `plugins/toolu/hooks/src/lifecycle/prompt-hints.ts`, `hooks/src/user-prompt-submit.ts`, `hooks/dist/user-prompt-submit.js`
- `tools/toolu-opencode/src/plugin/__tests__/jira-delivery.test.ts` (new), `jira-delivery.live.test.ts` (new)
- `tools/toolu-opencode/scripts/lib/rewrite.ts`, `scripts/__tests__/generate-surface.test.ts`, `generated/**`
- `plugins/jira/README.md`, `docs/jira/README.md`, `docs/opencode.md`

## Verification

- Real bundles, real `createTooluHooks`, the real HTTPS fixture with recorded Jira bodies, and real `/bin/sh`. No mocks.
- Failure and boundary cases: 401 on a read and 400 on a mutation exit 22 with the URL-only stderr line; no credentials exit 1 with setup help and no request; the token and `.env` secrets appear in no output, system line or log; a `.env` host override and a `.env` PAT are ignored (top level and nested checks); PATH without Bun; a project path with a space; a user-owned `jira.sh` named alone; nothing in compaction or when jira is deselected; Claude and Codex output unchanged.
- Docs synchronized and regenerated. The live host run is reported in the PR, apart from the fixture evidence. No real Jira tenant is contacted (spec Non-Goal 5).
- Delivery: scoped commits; `plan-ledger.js run <plan> --verify`; `toolu-review:review`; `verdict.js status` reporting `overall: ready`; push; PR to `main` starting `Closes Falconiere/toolu#351`; babysit handoff.

## Plan review

AC coverage was checked mechanically by extracting the ledger with `sed` and `jq`: AC-1 to AC-9 are all referenced, no reference dangles, and the dependency graph is acyclic (`delivery` and `live` wait for the hook, hint and skill steps).

Jev checked step/requirement alignment: 1.62/2 (0.67 on full coverage; it sees the inputs and checks but not test bodies). It named AC-9, the live host run, as the weakest (0.68).

- live: 🔵 consider: AC-9 depends on the pinned host binary and the scripted provider. Its ledger check sets `TOOLU_LIVE_OPENCODE=1`, so a missing binary fails rather than skips; the result is reported in the PR apart from the fixture evidence.
- delivery: 🔵 consider: AC-5's "zero requests from loading" needs the fixture proxy active in the adapter's own environment, not only in bash. The test sets the fixture env on the binding, so a request from startup would be recorded.
- toolu-hint: 🔵 consider: the OpenCode half of AC-6 is proven in `delivery`; this step only proves Claude/Codex are unchanged. Kept as is.

**Status:** Approved
