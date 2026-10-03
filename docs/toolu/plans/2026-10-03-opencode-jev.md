# OpenCode Jev startup, prompt reminders and typed judgments (OP-16) — Plan

**Date:** 2026-10-03 **Status:** Approved **Spec:** docs/toolu/specs/2026-10-03-opencode-jev-design.md **Topic:** Make Jev's mandate, reminder, skill and wrapper correct on OpenCode, with presence-only credential checks.

## Evidence and approach

A real-hook probe (`createTooluHooks`, temp project, `toolu` + `jev` selected) showed what already works and what does not.

- Already working, from #341, #343 and #345: the Jev mandate in `experimental.chat.system.transform`, one reminder per substantive prompt, and none for `ok`. `shell.env` exports `TOOLU_CONFIG_DIR` (data root) and `TOOLU_BUN`.
- Broken:
  - The mandate is duplicated in compaction context.
  - Both texts link the Claude-source `SKILL.md`.
  - The generated `jev-jev` skill is mislabelled.
  - Bun loads the project `.env` in hook children (`spawnEntry` spawns `[bun, bundle]` with the project as the working directory) and in the agent's `'<bun>' '<wrapper>'` command.

Reused as they are: core `detectHost` and `publishWrapper`, `createContextHooks`, `shellEnvFor` / `applyShellEnv`, the conformance `run` / `createSandbox` / `https-fixture` harness, and the live OpenCode harness in `tooling/src/opencode-host/` (as used by `context-delivery.live.test.ts`).

The design follows the approved spec. Jev's own hooks detect OpenCode, and the shared spawner gains `--no-env-file`. A `jev-jev` rewrite in the surface generator fixes the generated skill.

## Workstream summary

Jev hooks → OpenCode spawner → OpenCode delivery suite → generated skill → docs → live host proof → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "jev-hooks",
    "title": "Jev hooks on OpenCode: native skill reference, --no-env-file command for the published symlink, no mandate on compact; Claude/Codex unchanged",
    "ac_refs": ["AC-2", "AC-5"],
    "paths": [
      "plugins/jev/hooks/src/**",
      "plugins/jev/hooks/dist/**",
      "packages/toolu-core/src/host/**",
      "packages/toolu-core/src/startup/**"
    ],
    "input": "Real session-start and user-prompt-submit bundles under TOOLU_HOST_OVERRIDE=opencode in a sandbox (startup and compact stdin, symlink and user override) in the new opencode.test.ts, plus a Claude compact case; the existing Claude/Codex test files stay byte-identical to origin/main",
    "check": "bun test --timeout 60000 plugins/jev/hooks/src/__tests__/ && bun run check:plugin-bundles && git diff --quiet origin/main -- plugins/jev/hooks/src/__tests__/session-start.test.ts plugins/jev/hooks/src/__tests__/user-prompt-submit.test.ts plugins/jev/hooks/src/__tests__/jev.test.ts",
    "model": "inherit"
  },
  {
    "id": "spawner",
    "title": "Spawn OpenCode hook children with --no-env-file so a project .env never reaches them",
    "ac_refs": ["AC-4"],
    "depends_on": ["jev-hooks"],
    "paths": [
      "tools/toolu-opencode/src/bootstrap/**",
      "tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts",
      "plugins/jev/hooks/dist/**"
    ],
    "input": "Project with .env TYPESAFE_API_KEY=dotenv-secret; real Jev session-start and user-prompt-submit bundles through spawnEntry with an explicit env lacking the key (red before the flag)",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/bootstrap/__tests__/ tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts",
    "model": "inherit"
  },
  {
    "id": "delivery",
    "title": "OpenCode delivery suite: one mandate per system call, one reminder per prompt, none on compaction; typed judgment through the published wrapper with the shell.env environment; presence-only credentials",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["jev-hooks", "spawner"],
    "paths": [
      "tools/toolu-opencode/src/plugin/**",
      "tools/toolu-opencode/src/bootstrap/**",
      "tools/toolu-opencode/src/host/**",
      "plugins/jev/hooks/dist/**",
      "plugins/toolu/hooks/dist/**"
    ],
    "input": "Temp OpenCode project with toolu+jev selected; real createTooluHooks; HTTPS fixture for api.typesafe.ai (noul 0.92 and a 401); /bin/sh with an empty PATH; project .env with a secret key",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/plugin/__tests__/",
    "model": "inherit"
  },
  {
    "id": "skill",
    "title": "Generated jev-jev skill and problem-solving reference: active # OpenCode assignment, no CODEX_HOME, --no-env-file invocations",
    "ac_refs": ["AC-2"],
    "paths": [
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jev/skills/**"
    ],
    "input": "The real plugins/jev/skills/jev SKILL.md and references/problem-solving.md through the generator",
    "check": "bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document Jev on OpenCode (wrapper path, jev-jev skill, compaction, .env) and the spawner's --no-env-file",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["skill"],
    "paths": [
      "plugins/jev/README.md",
      "docs/jev/README.md",
      "docs/opencode.md",
      "tools/toolu-opencode/scripts/**",
      "tools/toolu-opencode/generated/**",
      "plugins/jev/skills/**"
    ],
    "input": "The edited READMEs and docs/opencode.md, regenerated into generated/resources",
    "check": "rg -q 'jev-jev' plugins/jev/README.md && rg -q 'jev-jev' docs/jev/README.md && rg -q -- '--no-env-file' docs/opencode.md && bun run check:opencode-surface",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Pinned OpenCode host: mandate in the system request, reminder in the user request, scripted bash runs the published wrapper against the HTTPS fixture",
    "ac_refs": ["AC-6"],
    "depends_on": ["delivery"],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "plugins/jev/**"
    ],
    "input": "opencode-ai@1.18.34 in an isolated profile with the scripted loopback provider and the HTTPS fixture; a separate TOOLU_LIVE_JEV=1 run with a real key, reported in the PR",
    "check": "TOOLU_LIVE_OPENCODE=1 bun test --timeout 600000 tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "live-jev",
    "title": "One real TypeSafe call through the OpenCode-published wrapper and shell.env environment, reported separately from the fixture evidence",
    "ac_refs": ["AC-6"],
    "depends_on": ["delivery"],
    "paths": [
      "tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
      "tools/toolu-opencode/src/**",
      "plugins/jev/**"
    ],
    "input": "The real TYPESAFE_API_KEY from the launch environment (the test fails, not skips, when TOOLU_LIVE_JEV=1 and the key is absent); real api.typesafe.ai; one noul question",
    "check": "TOOLU_LIVE_JEV=1 bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/jev-delivery.live.test.ts",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full repository gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["jev-hooks", "spawner", "delivery", "skill", "docs"],
    "paths": ["**"],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- `plugins/jev/hooks/src/jev/availability.ts`, `session-start.ts`, `user-prompt-submit.ts`, `hooks/dist/*.js`, `hooks/src/__tests__/opencode.test.ts` (new)
- `tools/toolu-opencode/src/bootstrap/spawn.ts`
- `tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts` (new), `jev-delivery.live.test.ts` (new)
- `tools/toolu-opencode/scripts/lib/rewrite.ts`, `scripts/__tests__/generate-surface.test.ts`, `generated/**`
- `plugins/jev/README.md`, `docs/jev/README.md`, `docs/opencode.md`

## Verification

- Real bundles, real `createTooluHooks`, the real HTTPS fixture and real `/bin/sh`. No mocks.
- Failure and boundary cases: the `.env` secret is never seen and never printed; a 401 exits 22 with no answer map; trivial or replayed prompts get no reminder; a user override keeps its bare invocation; Claude compact still re-injects.
- Docs are synchronized and regenerated. The live host run is recorded in the PR, separately from the real TypeSafe call.
- Delivery: scoped commits; `plan-ledger.js run <plan> --verify`; `toolu-review:review`; `verdict.js status` reporting `overall: ready`; push; PR to `main` starting `Closes Falconiere/toolu#350`; babysit handoff.

## Plan review

Jev checked step/requirement alignment: coverage 0.43 → 0.51 and paths 0.80 → 0.86 after the fixes. The residual uncertainty comes from Jev seeing check commands but not test bodies. The boundary inputs are named in each step's `input`.

- live: 🟡 should-fix: the real-TypeSafe half of AC-6 had no runnable check. Fixed: step `live-jev`, which fails when the key is absent.
- docs: 🟡 should-fix: `check:opencode-surface` reads the generator and skill sources, which were undeclared. Fixed: added to `paths`.
- jev-hooks: 🟡 should-fix: "existing tests unmodified" (AC-5) was not mechanically checked. Fixed: new cases live in `opencode.test.ts`; the check adds `git diff --quiet origin/main` over the existing test files.

**Status:** Approved
