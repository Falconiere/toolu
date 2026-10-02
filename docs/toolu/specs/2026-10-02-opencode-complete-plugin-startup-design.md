# OpenCode complete plugin startup — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Claude (epic worker, #342)   **Topic:** Run every selected plugin's startup entries in dependency order and compute readiness from this run's verified contributions (OP-08)

## Problem

At OpenCode plugin init, `bootstrapRuntime` runs one bundle per selected plugin: `hooks/dist/register.js` if it exists, otherwise `session-start.js` (`bootstrap/entrypoint.ts`). It throws away successful stdout, sends `{}` on stdin, and declares readiness as soon as *any* registry file or `toolu/.session-start-ready` exists under the data root (`bootstrap/readiness.ts`). This causes several failures:

- ts-, python- and rust-quality never run their second SessionStart entry (`check-toolu`).
- Startup context (toolu's session protocol, Jev's mandate) is lost.
- A stale marker from an earlier session, or one unrelated module, passes readiness.
- A partial registration passes readiness: `runRegisterHook` prints a stderr line and exits 0.
- A missing helper source passes readiness: `publishWrapper` returns `source-missing` silently.
- A disabled plugin's modules stay in the registry and keep running.
- Plugins run in selection order, which is BFS (ts-quality starts before toolu), and a dependency cycle goes undetected.

## Non-Goals

1. Delivering collected context to the model (`experimental.chat.system.transform`, prompt and compaction hooks). OP-07 (#341) owns it; this change only returns the context in a typed result.
2. Fixing HOME isolation, `TOOLU_PLUGIN_ROOT`, `shell.env` helper environment, or a data root shared by several projects. OP-09 (#343) owns these. When two projects with different selections share one data root, the last startup wins.
3. Installing or discovering generated skills, agents and commands (OP-11), and leaf-plugin behavior ports (OP-12…OP-25).
4. Per-plugin tool scoping. Any startup failure keeps the existing fail-closed deny-all `tool.execute.before` (#336); only the diagnosis improves.
5. Changing Claude Code or Codex behavior. The new report channel is inert unless `TOOLU_STARTUP_REPORT` is set, and only the OpenCode bootstrap sets it.
6. The legacy `clean-install-smoke` V2 route and the `permission.evaluate` adapter, apart from keeping their `bootstrapRuntime` call compiling.

## Architecture

**Entries come from `hooks.json`, the routing that Claude and Codex already use.** `pluginStartupEntries(pluginDir)` reads `<pluginDir>/hooks/hooks.json` with a strict Zod schema. It keeps every `SessionStart` group whose matcher covers `startup` (the matcher is absent, empty, `*`, or a `|`-separated list naming `startup`). Every hook in such a group must be `type: "command"` with a `command` byte-equal to `launcherCommand({ plugin: basename(pluginDir), event: "SessionStart", entry })`, the form that `check:hooks-json` already enforces, for the `entry` named in its `hooks/dist/<entry>.js` path. Entries run in file order. Edge cases:

- No `hooks.json` means no entries (brainstorm, delivery-flow).
- A hand-written or legacy `.sh` command is a plan failure.
- A missing bundle is a plan failure.

This replaces filename priority, the `.requires-native-register` marker and its staging in `bundle-plugins.ts`.

**Contributions come back as structured records.** A new core module `@toolu/core/startup` `report.ts` defines `StartupRecordSchema` and `reportStartup(record, env)`. When `env.TOOLU_STARTUP_REPORT` names a file, it appends one JSON line. If the append fails, it writes one stderr line and sets `process.exitCode = 1`, so a lost record can never look like success. With the variable unset, it does nothing. Three callers use it:

- `runRegisterHook` reports one `registry` record per declared module, with status `written`, `unchanged` or `failed` (`error` carries the failure). It also reports one `error` record for each other failure: a prune failure, or a thrown invalid spec.
- `publishWrapper` reports one `helper` record with its `PublishResult` status. It covers every helper, because `publishBunCli` wraps it and jev calls it directly.
- Nothing else changes in Claude/Codex output. Stdout, stderr and exit codes are byte-identical when the variable is unset.

**The bootstrap runs, verifies and records** (`tools/toolu-opencode/src/bootstrap/`):

1. **Plan:** compute `pluginStartupEntries` for every selected plugin before running anything.
2. **Prune (ownership):** read the ledger `<dataRoot>/toolu/startup-ledger.json`. For every plugin name in the catalog (`listPluginManifests(<repoRoot>/plugins)`) or in the ledger that is *not* selected:
   - Remove regular files (never symlinks) named `<name>@toolu__*.{js,sh}` in `<dataRoot>/toolu/{pre,post}-tools.d`.
   - Remove each ledger helper path that is still a symlink to its recorded source. Anything else at that path is now the user's: it is kept and reported as a diagnostic.
   - Modules of unknown specs are never touched.
3. **Run:** visit plugins in startup order (below), which puts dependencies first. A plugin is skipped as failed when its plan failed or any dependency failed. Each of its entries is spawned with:
   - the resolved Bun;
   - cwd at the project root;
   - stdin `{"hook_event_name":"SessionStart","source":"startup","cwd":<projectRoot>}`;
   - the existing bootstrap env plus `TOOLU_STARTUP_REPORT=<fresh temp file>` and `CLAUDE_PLUGIN_ROOT=<pluginDir>`.

   stdout and stderr are each bounded to 512 000 bytes. A per-entry deadline (default 120 s) and an optional `AbortSignal` kill the child.
4. **Verify (fresh, per plugin):** an entry succeeds only when all of these hold:
   - it exits 0;
   - its stdout is empty or strict SessionStart hook JSON (`hookSpecificOutput.{hookEventName:"SessionStart", additionalContext}` and/or `systemMessage`), with context bounded by `sessionContext`;
   - every report line parses;
   - no record is `failed`, an `error`, or a helper status other than `published`/`kept-user-file`;
   - each `registry` record's spec is the plugin's spec, its target sits inside `<dataRoot>/toolu/<event dir>/<spec>__<name>.js`, its source sits inside the plugin directory, and the target's bytes equal the source's;
   - each `published` helper is a symlink inside the data root whose link text is its source, and that source sits inside the plugin directory.

   `kept-user-file` is a diagnostic. stderr is a diagnostic. Readiness never reads a file this run did not report.
5. **Clean up stale helpers, then write the ledger:** a ready plugin's ledger helpers that this run no longer published are removed under the rule in step 2. The ledger is rewritten atomically (unique tmp file, then rename). A failed plugin keeps the union of its old and new helpers.
6. **Result:** `ready` only when no plugin failed and no prune step failed. `ready` carries every verified artifact, per-plugin entry outcomes (context, `systemMessage`) for OP-07, and diagnostics. `not-ready` carries one reason listing each `plugin/entry: cause`, capped at 4 000 characters.

**Startup order.** `startupOrder(plugins)` (`bootstrap/order.ts`) orders the selected manifests topologically: a depth-first search over names, and over each plugin's dependencies, in sorted order, with dependencies first. A back edge makes startup NotReady with `plugin dependency cycle: a -> b -> a`. A dependency missing from the given set makes it NotReady with `<plugin> requires unselected dependency <dep>`. Selection (`select/resolve.ts`) is unchanged: it still computes the closure and reports missing dependencies. Its order also feeds `generate-surface`, whose committed catalog is in name order, and OP-10 (#344) is rewriting that script in parallel.

**Containment checks** compare real paths (`realpathSync`) on both sides. The data root, the plugin directory and the temp directory may sit behind symlinks (an npm `node_modules` link, a symlinked macOS `/var`).

**Enforcement.** `prepareEnforcement` passes `AbortSignal.timeout(180_000)` as the total startup budget. `Enforcement.ready` gains `plugins` and `diagnostics`. `createTooluHooks` logs `toolu: ready (<p> plugins, <n> startup artifacts)` and one `info` line per diagnostic, capped at 20. The deny-all path is unchanged.

**Marker removal.** toolu's housekeeping stops touching `toolu/.session-start-ready`; nothing else reads it. `.gate-preset-notice-v6` keeps its own purpose.

**Reuse.** `launcherCommand` (`@toolu/core/launcher`), `sessionContext` and `MAX_CONTEXT_CHARS` (`@toolu/core/startup`), `registryDirName`, `registryFileName` and `REGISTRY_DIRS` (`@toolu/core/registry`), `listPluginManifests`, `resolveBunExecutable`, `opencodeRegistryRoot`.

## Interfaces / Schema

```ts
// packages/toolu-core/src/startup/report.ts (exported from @toolu/core/startup)
export const STARTUP_REPORT_ENV = "TOOLU_STARTUP_REPORT";
export const StartupRecordSchema = z.discriminatedUnion("kind", [
  z.strictObject({ kind: z.literal("registry"), spec: z.string().min(1), name: z.string().min(1),
    event: z.enum(REGISTRY_EVENTS), source: z.string().min(1), target: z.string().min(1),
    status: z.enum(["written", "unchanged", "failed"]), error: z.string().optional() }),
  z.strictObject({ kind: z.literal("helper"), plugin: z.string().min(1), source: z.string().min(1),
    path: z.string().min(1).optional(),
    status: z.enum(["published", "kept-user-file", "link-failed", "unwritable", "source-missing"]) }),
  z.strictObject({ kind: z.literal("error"), origin: z.string().min(1), message: z.string() }),
]);
export type StartupRecord = z.infer<typeof StartupRecordSchema>;
export function reportStartup(record: StartupRecord, env?: HostEnv): void;

// tools/toolu-opencode/src/bootstrap/entrypoint.ts
export type StartupEntry = { name: string; bundle: string };
export function pluginStartupEntries(pluginDir: string):
  | { ok: true; entries: StartupEntry[] } | { ok: false; reason: string };

// tools/toolu-opencode/src/bootstrap/result.ts
export type EntryOutcome = { entry: string; additionalContext?: string; systemMessage?: string };
export type PluginStartup = { plugin: string; entries: EntryOutcome[]; artifacts: string[] };
export type ReadyResult = { status: "ready"; artifacts: string[]; plugins: PluginStartup[]; diagnostics: string[] };
export type NotReadyResult = { status: "not-ready"; reason: string };

// tools/toolu-opencode/src/bootstrap/runtime.ts — options gain `signal`
export type BootstrapRuntimeOptions = { repoRoot: string; dataRoot?: string; projectRoot: string;
  plugins: PluginManifest[]; env?: Record<string, string>; isolatedHome?: string;
  deadlineMs?: number; signal?: AbortSignal };

// tools/toolu-opencode/src/plugin/enforcement.ts
export type Enforcement =
  | { status: "ready"; before: ToolBefore; artifacts: string[]; plugins: PluginStartup[]; diagnostics: string[] }
  | { status: "not-ready"; reason: string };
```

Ledger (`<dataRoot>/toolu/startup-ledger.json`, strict Zod; an unreadable or invalid ledger counts as empty and adds a diagnostic):

```json
{ "version": 1, "plugins": { "context7": { "spec": "context7@toolu",
  "helpers": [{ "path": "<dataRoot>/context7/search.sh", "source": "<pluginsRoot>/context7/hooks/dist/search.js" }] } } }
```

`@toolu/opencode/bootstrap` exports `bootstrapRuntime`, `BootstrapRuntimeOptions`, `pluginStartupEntries` and the result types. It drops `collectBootstrapArtifacts`, `evaluateBootstrapReadiness` and `pluginBootstrapScript`, which are internal to the bootstrap and replaced. Their only consumers are in this repository.

## Failure modes and edge cases

| Input / state | Observable behavior | Propagation |
|---|---|---|
| Selected plugin's `hooks.json` invalid JSON or schema | NotReady: `<plugin>: invalid hooks.json …`; its dependents are skipped | Converted |
| SessionStart command is not the generated launcher (e.g. `bash …/register.sh`) | NotReady: `<plugin>: unsupported SessionStart command` | Converted |
| Declared bundle `hooks/dist/<entry>.js` missing | NotReady: `<plugin>/<entry>: missing startup bundle`; nothing of that plugin runs | Converted |
| Matcher lacks `startup` (e.g. `compact` only) | Entry not run at init | Handled |
| Entry exits ≠ 0, times out, exceeds 512 000 output bytes, cannot spawn | NotReady with exit code plus stderr/stdout excerpt, `timed out`, `output exceeded`, or spawn error | Converted |
| `AbortSignal` aborts mid-run | Child killed; NotReady `startup cancelled` | Converted |
| stdout not JSON, or JSON outside the strict hook shape | NotReady: `<plugin>/<entry>: invalid startup output` | Converted |
| Report line not JSON or fails the schema | NotReady: `invalid startup report` | Converted |
| Registry record `failed` (one of two modules), or prune `error` | NotReady naming the module path and error; the written module stays | Converted |
| Registry record for another spec, or target/source outside its roots | NotReady: `contribution outside <plugin>` | Converted |
| Target bytes differ from bundle after the run (concurrent other version) | NotReady: `<target> is not the current bundle` | Converted |
| Helper `source-missing` / `link-failed` / `unwritable` | NotReady naming the helper | Converted |
| Helper `kept-user-file` (user's own `jev.sh`) | Ready; diagnostic `kept user file <path>` | Recovered |
| Stale `.session-start-ready`, unrelated module, old-version module of a selected plugin on disk | Ignored by readiness. The old-version module is rewritten (`written`) and then verified | Handled |
| Dependency failed (e.g. toolu) | Dependents: `<plugin>: skipped, dependency toolu failed` | Converted |
| Plugin disabled since the last startup | Its `<name>@toolu__*` registry files and its still-owned helper symlinks are removed | Recovered |
| Disabled plugin's helper replaced by a user file | Kept; diagnostic | Recovered |
| Removal of a disabled module or helper throws | NotReady naming the path | Converted |
| Ledger unreadable or invalid | Treated as empty; diagnostic. Registry prefix pruning still runs | Recovered |
| Selection resolves to zero plugins | Ready with no plugin startups; native core gates still enforce | Handled |
| Dependency cycle among the given plugins | NotReady: `plugin dependency cycle: a -> b -> a` → deny-all | Converted |
| Given plugin depends on a plugin not in the given set | NotReady: `<plugin> requires unselected dependency <dep>` | Converted |
| Repeated startup with unchanged inputs | Ready again with the same artifacts. Registry records `unchanged`; files and the ledger are byte-identical | Handled |
| Two instances start at once on one data root (same selection) | Both are ready. Registry and helper writes are atomic renames, and the ledger is written through a unique tmp file plus rename, so the last writer leaves identical content | Handled |
| Report append fails inside an entry | The entry exits 1 → NotReady | Converted |
| Claude/Codex (`TOOLU_STARTUP_REPORT` unset) | No file written; output and exit codes unchanged | Handled |

## Acceptance criteria

- **AC-1:** Given all 16 catalog plugins selected in a fresh project with the real committed bundles, `bootstrapRuntime` returns `ready`. Its `plugins` list follows dependency order (toolu before every plugin that depends on it) and has one record per startup entry declared in `hooks.json`, including both `register` and `check-toolu` for the ts-, python- and rust-quality plugins. The data root holds exactly the expected registry modules (ast-grep's search-nudge and byte-savings, and each quality plugin's module), byte-equal to their bundles, and the expected helper symlinks (context7, exa-search, jev, jira, agent-browser, statusline, toolu-review). The toolu session protocol and Jev's mandate appear as `additionalContext`.
- **AC-2:** Given a data root holding a stale `toolu/.session-start-ready`, an unrelated module `custom@local__extra.js`, and an old-version module of a selected plugin, a selected plugin whose startup cannot complete yields `not-ready`:
  - (a) `ast-grep` with `byte-savings.js` removed, which is a partial registration;
  - (b) `context7` with `search.js` removed, which is a missing helper source;
  - (c) `ts-quality` with `check-toolu.js` removed, which is a missing entry bundle.

  The reason names the plugin and the cause.
- **AC-3:** Given two consecutive startups with unchanged inputs, both are `ready` with identical artifact lists. The second reports every registry module `unchanged`, and the registry files and the ledger are byte-identical, with unchanged registry mtimes.
- **AC-4:** Given a startup with toolu, ast-grep and context7, then a startup with only toolu, the ast-grep registry modules and the context7 helper symlink are gone. A user regular file placed at a disabled helper path is kept and reported. `custom@local__extra.js` is untouched.
- **AC-5:** Given a selected plugin whose entry exits non-zero, stalls past its deadline, is cancelled through `AbortSignal`, floods stdout, prints non-JSON, or writes an invalid or foreign report record, startup is `not-ready`. Through `createTooluHooks`, every guarded tool call is refused with `toolu: not ready: <plugin>/<entry>: <cause>`.
- **AC-6:** Given the real selection closure of `delivery-flow` in its BFS order, startup runs every dependency before its dependent. The `plugins` result is exactly `brainstorm, toolu, pr-babysit, toolu-review, delivery-flow`, because names and each plugin's dependencies are walked in sorted order. Fixture plugins where `a` depends on `b` and `b` depends on `a` are `not-ready` with `plugin dependency cycle: a -> b -> a`, and neither entry runs.
- **AC-7:** With `TOOLU_STARTUP_REPORT` unset, `runRegisterHook` and `publishWrapper` create no report file, and the committed SessionStart bundles' Claude-host stdout and exit codes are unchanged (existing plugin tests stay green). With it set, they append records that `StartupRecordSchema` accepts, including a `failed` registry record for an unreadable bundle. An unwritable report path makes the entry exit 1.
- **AC-8:** On the pinned `opencode-ai@1.18.34` host:
  - (a) A local shim with all 16 plugins enabled logs exactly one `toolu: ready (16 plugins …)` and leaves the expected registry modules and helpers in the project data root.
  - (b) A catalog copy whose ts-quality `post-tool-use.js` is missing logs `toolu: not ready` naming ts-quality. A scripted bash `touch` is refused, and its file is never created.
  - (c) A second session after ast-grep is disabled leaves no ast-grep registry modules.

## Acceptance evidence

| AC | Real input / fixture | Expected observation | Check |
|---|---|---|---|
| AC-1 | Repo `plugins/` (all 16), temp project and data root | `ready`; exact module and helper sets; bytes equal; order; contexts present | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-catalog.test.ts` |
| AC-2 | Temp copy of the named plugins with one bundle deleted; pre-seeded marker, unrelated module and stale module | `not-ready`; reason contains plugin and cause | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-readiness.test.ts`; conformance `bootstrap-readiness` suite |
| AC-3 | Same project and data root bootstrapped twice | Identical artifacts; `unchanged`; same bytes and mtimes | `startup-readiness.test.ts` |
| AC-4 | Selection file rewritten between two runs; a user file placed at the helper path | Files removed or kept as stated; diagnostic present | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-ledger.test.ts` |
| AC-5 | Fixture plugins with generated launcher `hooks.json` and small real bundles | `not-ready` with cause; deny-all message | `bun test tools/toolu-opencode/src/bootstrap/__tests__/bootstrap-failure.test.ts`, `plugin/__tests__/hooks.test.ts` |
| AC-6 | Repo plugins via `selectPluginsByEnabledNames(…, ["delivery-flow"])`; two fixture plugins forming a cycle, each with a marker-writing bundle | Topological order in the `plugins` result; cycle reason; no marker written | `bun test tools/toolu-opencode/src/bootstrap/__tests__/startup-order.test.ts` |
| AC-7 | Real core functions in subprocesses, with and without the variable | No file vs schema-valid records; exit 1 on unwritable report | `bun test packages/toolu-core/src/startup/__tests__/report.test.ts`, existing plugin startup tests |
| AC-8 | Pinned host, isolated profile, scripted loopback provider | Log lines, tool states, data-root files | `bun run smoke:opencode-entry` (new `entry.full-startup`, `entry.startup-failure` and `entry.startup-disable` scenarios) |

## Documentation impact

- `docs/opencode.md`: the bootstrap paragraph (every `hooks.json` SessionStart entry, dependency order, record-based readiness, the ready log line, disabled cleanup, the ledger) and the scoped-cleanup bullet.
- `docs/registry.md`: the Registering paragraph (report records under `TOOLU_STARTUP_REPORT`).
- `AGENTS.md`: key-file row for `@toolu/core/startup` (the startup report channel).
- `tools/toolu-opencode/README.md`, if it describes bootstrap selection.
- Release note through the conventional-commit body.

## Open Questions

None blocking. The following were decided unattended from the issue, the epic and the code (Jev-assisted):

- **Blocking scope:** keep the deny-all. Per-plugin tool scoping does not exist yet, and the host fails open on init errors.
- **User-owned helper file:** a diagnostic, not a failure. User overrides at helper paths are a documented feature.
- **Empty selection:** ready, because the native core gates still enforce.
- **The marker:** removed.
- **Total startup budget:** 180 s.

## Spec review

- Failure modes: 🟡 should-fix (fixed): two concurrent startups on one data root were not stated. Added a row.
- Architecture: 🟡 should-fix (fixed): topological order inside `select/resolve.ts` would reorder `generate-surface`'s committed catalog, which OP-10 is rewriting in parallel. Ordering moved into `bootstrap/order.ts`, and AC-6 was restated against the startup result.
- Architecture: 🔵 consider (fixed): containment checks must survive symlinked roots. They now compare real paths.
- Jev (requirement/evidence alignment, `review-state.md`): issue bullet 1 → 1.99/2, bullet 2 → 1.87/2, bullet 3 → 1.99/2. Every issue acceptance bullet maps to an observable AC.

**Status:** Approved
