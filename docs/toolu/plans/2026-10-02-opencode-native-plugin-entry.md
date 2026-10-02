# OpenCode native plugin entry (OP-02) — Plan

**Date:** 2026-10-02   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-02-opencode-native-plugin-entry-design.md   **Topic:** Replace the V2 `Plugin.define` entry with a default `PluginModule` whose `server` returns `tool.execute.before` hooks and fails closed.

## Evidence and approach

**Recalled.**
- `6990a41a`: the OP-01 host facts, which are fail-open init, `before`-throw denies, and only `server` being called for a default `PluginModule`.
- `ce014dd7`: the V2/docs mismatch.

**Inspected.**
- `tools/toolu-opencode/src/plugin/toolu.ts`: the V2 entry. Its setup path (preflight, selection, bootstrap) is reused as is.
- `src/adapter/evaluate.ts`: the nine native gates over `dispatchPreTool`. Its gate list moves into the shared `createGateDecider`.
- The pinned SDK `dist/index.d.ts` (`Plugin`, `PluginModule`, `Hooks`).
- The pinned host loader at `v1.18.34`:
  - `plugin/index.ts`: `applyPlugin`, `getLegacyPlugins`, `dispose` called in the finalizer.
  - `plugin/shared.ts`: `readV1Plugin`, `resolvePackageEntrypoint` (`./server`, then `main`), `resolvePluginTarget` (`Npm.add`).
  - `project/project.ts`: worktree `/` for a non-VCS project.
- The live harness under `tooling/src/opencode-host/` (`openSession`, `runHost`, `toolStates`, `resolveHostBinary`).

**Verified with scratch runs on the pinned host** (deleted before commit):
- An awaited `client.app.log` during init appears in `--print-logs` stderr.
- `plugin: ["<name>@file:/abs.tgz"]` installs and loads a packed default `PluginModule` through the npm route.

**Approach.** Follow the spec's architecture. The live smoke reuses the OP-01 harness. It packs a temp copy of the package, using `stagePlugins` and then `bun pm pack --ignore-scripts`, so the working tree never changes.

## Workstream summary

1. Adapter: shared gate decider, then the `tool.execute.before` mapping and handler.
2. Entry: once-guard, host binding, enforcement, hooks and module, with hermetic tests.
3. Package and wiring: drop `@opencode/plugin`, update the exports, knip and pack inventory.
4. Live smoke against the pinned host.
5. Docs, then the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "adapter",
    "title": "Shared createGateDecider in evaluate.ts; adapter/tool-before.ts with mapToolCall, createToolBeforeHandler, createDenyAllToolBefore; colocated tests over a real temp project and the repo's plugins",
    "ac_refs": ["AC-6"],
    "paths": [
      "tools/toolu-opencode/src/adapter/**",
      "plugins/toolu/**"
    ],
    "input": "Real OpenCode arg shapes recorded by the OP-01 probes: bash {command, description}, edit {filePath, oldString, newString}, write {filePath, content}; temp project with .env and protectedFiles block",
    "check": "bun test --timeout 60000 tools/toolu-opencode/src/adapter/__tests__",
    "model": "inherit"
  },
  {
    "id": "entry",
    "title": "src/plugin/{once,context,enforcement,hooks,toolu}.ts: default PluginModule, once-per-directory guard released by dispose, bindHostContext, prepareEnforcement, createTooluHooks (never throws, readiness diagnostic, deny-all); hermetic tests including the export shape and the cast/any source scan",
    "ac_refs": ["AC-4", "AC-5", "AC-6", "AC-7"],
    "depends_on": ["adapter"],
    "paths": [
      "tools/toolu-opencode/src/plugin/**",
      "tools/toolu-opencode/src/adapter/**",
      "tools/toolu-opencode/src/{preflight,select,bootstrap,host,inventory}/**",
      "plugins/toolu/**",
      "tsconfig.json"
    ],
    "input": "Temp git project with .env and .opencode/toolu.config.json protectedFiles block, the repo's real plugins/ tree and real Bun bootstrap; invalid options {repoRoot: 42}; missing repo root; a preparation that throws",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__ && bun run typecheck",
    "model": "inherit"
  },
  {
    "id": "package",
    "title": "Remove @opencode/plugin dependency, add ./adapter/tool-before export, refresh bun.lock, check-workspace default entry, knip and pack inventory",
    "ac_refs": ["AC-4", "AC-6"],
    "depends_on": ["entry"],
    "paths": [
      "tools/toolu-opencode/package.json",
      "bun.lock",
      "tooling/src/check-workspace.ts",
      "tooling/src/pack-inventory.ts",
      "knip.json"
    ],
    "input": "The workspace package manifest and the bun pm pack --dry-run file list",
    "check": "bun install --frozen-lockfile && test \"$(jq -r '.dependencies[\"@opencode/plugin\"] // \"absent\"' tools/toolu-opencode/package.json)\" = absent && bun run test:workspace && bun run test:pack && bun run knip",
    "model": "inherit"
  },
  {
    "id": "live",
    "title": "Live smoke: tooling/src/opencode-host/scenarios-entry.ts (entry.npm-root, entry.local-shim, entry.init-failure, entry.both-routes) and tooling/src/opencode-entry-smoke.ts; root script smoke:opencode-entry",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-7"],
    "depends_on": ["package"],
    "paths": [
      "tooling/src/opencode-entry-smoke.ts",
      "tooling/src/opencode-host/scenarios-entry.ts",
      "tooling/src/opencode-host/**",
      "tools/toolu-opencode/src/**",
      "tools/toolu-opencode/package.json",
      "plugins/toolu/**",
      "package.json"
    ],
    "input": "Pinned opencode-ai@1.18.34 from the version-keyed cache, isolated profiles, scripted loopback provider, packed @toolu/opencode tarball, project node_modules symlink and .opencode/plugins/toolu.ts shim",
    "check": "bun run smoke:opencode-entry",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Docs: opencode-host-contract status, portable-core entry and export rows, opencode.md pins/wiring/entry/diagnostic and smoke, conformance-report SDK pin note",
    "depends_on": ["live"],
    "paths": [
      "docs/opencode-host-contract.md",
      "docs/portable-core.md",
      "docs/opencode.md",
      "docs/conformance-report.md"
    ],
    "input": "The implemented entry and the live smoke output",
    "check": "bun run test:portable-core && ! grep -n '@opencode/plugin@2.0.12\\|Plugin.define' docs/opencode.md docs/portable-core.md docs/opencode-host-contract.md",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate",
    "ac_refs": ["AC-4", "AC-5", "AC-6"],
    "depends_on": ["docs"],
    "paths": ["**"],
    "input": "The whole branch",
    "check": "bun run test",
    "model": "inherit"
  }
]
```

## Critical files

- Create:
  - `tools/toolu-opencode/src/adapter/tool-before.ts`
  - `tools/toolu-opencode/src/adapter/__tests__/tool-before.test.ts`
  - `tools/toolu-opencode/src/plugin/{once,context,enforcement,hooks}.ts`
  - `tools/toolu-opencode/src/plugin/__tests__/{toolu,hooks,once}.test.ts`
  - `tooling/src/opencode-host/scenarios-entry.ts`
  - `tooling/src/opencode-entry-smoke.ts`
- Modify:
  - `tools/toolu-opencode/src/plugin/toolu.ts`
  - `tools/toolu-opencode/src/adapter/evaluate.ts`
  - `tools/toolu-opencode/package.json`
  - `bun.lock`
  - root `package.json` (script)
  - the four docs listed in the `docs` step: `docs/opencode-host-contract.md`, `docs/portable-core.md`, `docs/opencode.md`, `docs/conformance-report.md`
- Delete: the scratch experiment directory `tooling/src/scratch-336/` (never committed).
- Read by checks, not edited: `knip.json`, `tooling/src/check-workspace.ts`, `tooling/src/pack-inventory.ts`, the OP-01 harness modules under `tooling/src/opencode-host/`, and the existing V2 adapter tests (`src/adapter/__tests__/evaluate.test.ts`, `permission-map.test.ts`, `result.test.ts`). The shared decider refactor must keep those tests green unchanged.

## Verification

- **End to end.** `bun run smoke:opencode-entry` runs four scenarios on the real pinned host:
  - `entry.npm-root`: one ready line; the protected `.env` stays unchanged.
  - `entry.local-shim`: one ready line; the `.env` write is denied and the allowed `touch` succeeds.
  - `entry.init-failure`: one not-ready line; the tool is denied and its marker is absent.
  - `entry.both-routes`: one ready line and one duplicate line; `.env` unchanged.
- **Hermetic.** The plugin and adapter tests use real temp projects, the real `plugins/` tree and real Bun bootstrap. The typecheck runs against the pinned SDK.
- **Failure and boundary.** Covered cases are:
  - a preparation that throws;
  - invalid options;
  - a missing root;
  - a nonexistent `repoRoot` on the real host;
  - invalid tool args;
  - an `ask` decision becoming a deny;
  - duplicate loads and a release on `dispose`.
- **Read-only checks.** `tooling/src/check-workspace.ts` and `tooling/src/pack-inventory.ts` need no edit: `default.id === "toolu"` holds for the `PluginModule`, and the new sources ship under `files: ["src"]`. The `package` step's checks still read them.
- **Docs.** The `test:portable-core` doc checks pass, and no V2 entry claim remains in the touched docs.
- **Full gate.** `bun run test`.
- **Delivery.**
  1. Scoped commits on `feat/336-opencode-replace-the-v2-entrypoint`.
  2. `plan-ledger.js run <this plan> --verify`.
  3. `toolu-review:review`, recording v2 state.
  4. `verdict.js status` reports `overall: ready`.
  5. Push, then open a PR to `main` whose body starts with `Closes Falconiere/toolu#336` / `Part of Falconiere/toolu#334`.
  6. `pr-babysit:babysit`.

## Plan review

- AC-to-step coverage: every spec AC (AC-1 through AC-7) is mapped, with no dangling refs (checked by a script over the spec and ledger). Jev scored each AC's step 0.91–0.96.
- Gate step: 🟡 should-fix (resolved): it claimed the live ACs, but `bun run test` does not run the live host. Its refs are now AC-4/5/6 only, and AC-1/2/3/7 belong to the `live` step.
- Docs step: 🟡 should-fix (resolved): it claimed ACs its check cannot prove. Its `ac_refs` were dropped, and the check keeps the doc-sync and V2-claim scan.
- Critical files: 🟡 should-fix (resolved): the conditional "only if needed" edits were ambiguous. The files are now named as read-only, with the reason, and the scratch directory deletion is explicit. Jev's order question then scored 0.36 because of unnamed read files, which are now named.
- Dependency order: linear (adapter → entry → package → live → docs → gate), each depending only on earlier steps (checked by script).

## Deviations

- `prepareEnforcement` and `resolveRepoRoot` take an optional `findBundled` lookup, which defaults to the package's own `plugins/`. A dev tree can carry a prepack-staged `tools/toolu-opencode/plugins/`, which made the "no repo root" test depend on the environment; the test now passes `() => undefined`.
- The scenario harness's `SessionOptions.scripts` also accepts a function of the project path, so the live smoke scripts absolute `filePath` args the way a real model sends them.
- Full gate on this host. The run used a deduplicated `PATH` (without `/bin` and `/sbin`), and every `test:ts` stage was run.
  - Five unit tests fail only because the runner is root, which ignores chmod. They are `publish.test.ts` "refuses the link", jev `session-start.test.ts` "refuses the link", and the ts/python/rust `read-failure.test.ts` "unreadable" cases. They fail identically on a clean `origin/main` worktree.
  - The PATH-based golden tests fail on `origin/main` too, because this host lists both `/bin` and `/usr/bin` (merged `/usr`) and has a dangling `grub-ntldr-img` link. All 338 pass with the deduplicated `PATH`.
  - `bench:shell --assert` reports a parse p99 of about 210 µs against a 100 µs budget at load average ≈15. This branch does not change `packages/toolu-core`. CI is the authority for these.
