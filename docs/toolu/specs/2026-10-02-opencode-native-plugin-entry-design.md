# OpenCode native plugin entry — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Claude (epic worker, #336)   **Topic:** Replace the V2 `Plugin.define` entry of `@toolu/opencode` with the documented plugin function (OP-02)

## Problem

`tools/toolu-opencode/src/plugin/toolu.ts` imports a runtime `Plugin` value from the superseded `@opencode/plugin@2.0.12` and calls `Plugin.define`, `ctx.permission.hook("evaluate")` and `ctx.location`. The pinned host (`opencode-ai@1.18.34`, contract in `docs/opencode-host-contract.md`) loads a function `(input: PluginInput, options?) => Promise<Hooks>` or a default `PluginModule { id, server }`. On the documented host the current entry enforces nothing. The host also fails open when plugin init throws (`load.init-throw`), so a careless port would silently drop every gate.

## Non-Goals

1. Tool normalization beyond today's coverage (bash, edit, write). Read, grep, glob, apply_patch, MCP and task routing belong to OP-03 (#337).
2. Per-gate-class `ask` degradation and advisories. OP-05 (#339) owns them. Here every non-allow decision denies.
3. Post-tool dispatch (`tool.execute.after`, OP-06), context delivery (OP-07), complete startup and fresh readiness (OP-08), path and env resolution (OP-09).
4. Migrating the V2-shaped `permission.evaluate` adapter (`adapter/evaluate.ts`, `adapter/permission-map.ts`) or its conformance and smoke consumers. They stay as they are until OP-03/04/05 replace them.
5. Rewriting the install docs. OP-29 (#363) owns that. This change only corrects statements it makes false.
6. Making live-host acceptance mandatory in CI. OP-28 (#362) owns that.

## Architecture

The root module default-exports one `PluginModule`, with no other runtime exports:

```ts
const tooluModule: PluginModule = { id: "toolu", server: tooluServer };
export default tooluModule;
```

The decisive trade-off is this export shape. When a default `PluginModule` is present, the pinned host (`getServerPlugin`/`readV1Plugin` in the 1.18.34 loader, probe `load.module-default`) calls only `server` and ignores named exports. A named-function-only module would make any future helper export a second "plugin" (`load.helper-export`). `check-workspace.ts` already reads `default.id === "toolu"`.

`tooluServer(input, options)`:

0. **Once per instance directory.** A process-wide guard (`globalThis[Symbol.for("toolu.opencode.instances")]`, a `Set` of directories shared across module copies, such as the npm cache copy and a local shim's copy) admits the first `server` call for a directory. A second call for the same directory, for example when the npm spec and a local shim are both configured, returns `{}` and logs `toolu: duplicate load skipped for <directory>`. The admitted instance releases the directory in its `dispose` hook, which the host calls from the instance finalizer, so a re-created instance enforces again.
1. **Bind the host context** in `bindHostContext(input, options, process.env)`:
   - `directory` is the instance directory, used as the cwd for gate dispatch.
   - `projectRoot` is `input.worktree`, or `input.directory` when the worktree is `/`. The host sets the worktree to `/` for a non-VCS global project.
   - `options` are validated with Zod.
   - `log` writes through `input.client.app.log`.
2. **Prepare enforcement** with `prepareEnforcement(binding)`, which reuses the old setup path unchanged: `runPreflight`, then `selectPluginsWithDependencies`, then `opencodeDataRoot` and `bootstrapRuntime`. It returns `{ status: "ready", before }` or `{ status: "not-ready", reason }`. The repo root resolves as before: option `repoRoot`, then `TOOLU_REPO_ROOT`, then `TOOLU_ROOT`, then the bundled `plugins/` beside the package.
3. **Never throw.** `createTooluHooks` wraps step 2 in try/catch. Any thrown error becomes `not-ready` with the error message.
4. **Emit one readiness diagnostic** through `client.app.log`, with service `toolu`:
   - `info`, message `toolu: ready (<n> bootstrap artifacts)`, when ready;
   - `error`, message `toolu: not ready: <reason>; every tool call is denied`, when not.

   The call is bounded by a 5 s race, and its own failure is swallowed. It is a diagnostic, never the enforcement path.
5. **Return `Hooks`** with exactly `"tool.execute.before"` and `dispose`:
   - Ready: the gate handler.
   - Not ready: a deny-all handler that throws `toolu: not ready: <reason>` for every tool, so the host refuses the call before any side effect (`deny.*` probes).

### Gate handler

`createToolBeforeHandler`, in `src/adapter/tool-before.ts`:

1. Map the call with `mapToolCall({tool, sessionID, callID}, args, ctx)`, which uses Zod:
   - `bash` with `{command: string}` becomes `Bash { command }`.
   - `edit` and `write` with `{filePath: string}` become `Edit` or `Write { file_path }`.
   - Every other tool is skipped.
   - A gated tool with missing or invalid args is denied.
2. Run the same nine native gates in process (`dispatchPreTool`) through a shared `createGateDecider` that `evaluate.ts` and `tool-before.ts` both use, so there is no duplicate gate list.
3. Act on the decision:
   - `allow` and `advisory` return normally.
   - `deny`, `ask`, `post_block` and `runtime_failure` throw `Error(reason)`.

Ask fails closed until OP-05, because the host has no ask channel (`permission.ask-hook` unsupported) and a toolu allow must never bypass the user's rules.

Host SDK types stay inside `tools/toolu-opencode`. The entry imports `@opencode-ai/plugin` type-only, so it needs no runtime dependency. The pinned devDependency `@opencode-ai/plugin@1.18.34` provides the declarations, and the host provisions the SDK at runtime. The runtime dependency `@opencode/plugin@2.0.12` is removed.

**Layout.** Each file stays at 300 lines or fewer and each function at 60 or fewer:

- `src/plugin/toolu.ts`: the module and `server`.
- `src/plugin/hooks.ts`: `createTooluHooks`, the readiness diagnostic, the deny-all hook.
- `src/plugin/once.ts`: `claimInstance(directory)` and `releaseInstance(directory)` over the global guard.
- `src/plugin/enforcement.ts`: `prepareEnforcement`, `resolveRepoRoot`.
- `src/plugin/context.ts`: `bindHostContext`, options schema, project root rule.
- `src/adapter/tool-before.ts`: `mapToolCall`, `createToolBeforeHandler`, `createDenyAllToolBefore`.

## Interfaces / Schema

```ts
// src/plugin/context.ts
export const TooluPluginOptionsSchema = z.looseObject({ repoRoot: z.string().min(1).optional() });
export type HostBinding = {
  directory: string;             // PluginInput.directory
  projectRoot: string;           // worktree, or directory when worktree === "/"
  options: { repoRoot?: string };
  optionsError?: string;         // set when options fail the schema
  env: Record<string, string>;   // process.env without undefined values
  log: (level: "info" | "error", message: string) => Promise<void>;
};
export function bindHostContext(input: PluginInput, options: PluginOptions | undefined, env: NodeJS.ProcessEnv): HostBinding;

// src/plugin/enforcement.ts
export type ToolBefore = NonNullable<Hooks["tool.execute.before"]>;
export type Enforcement =
  | { status: "ready"; before: ToolBefore; artifacts: string[] }
  | { status: "not-ready"; reason: string };
export function prepareEnforcement(binding: HostBinding): Promise<Enforcement>;

// src/plugin/once.ts
export function claimInstance(directory: string): boolean; // false when already claimed
export function releaseInstance(directory: string): void;

// src/plugin/hooks.ts
export function createTooluHooks(binding: HostBinding): Promise<Hooks>; // never rejects

// src/adapter/tool-before.ts
export type ToolCall = { tool: string; sessionID: string; callID: string };
export function mapToolCall(call: ToolCall, args: unknown, ctx: PermissionContext): PermissionMapping;
export function createToolBeforeHandler(opts: PermissionEvaluateHandlerOptions): ToolBefore;
export function createDenyAllToolBefore(reason: string): ToolBefore;

// src/adapter/evaluate.ts (shared, behavior unchanged for permission.evaluate)
export function createGateDecider(opts: PermissionEvaluateHandlerOptions):
  { ok: true; decide(request: Record<string, unknown>): Promise<Decision> } | { ok: false; reason: string };
```

Package changes (`tools/toolu-opencode/package.json`):

- Remove `dependencies["@opencode/plugin"]`.
- `main` and `exports["."]` stay `./src/plugin/toolu.ts`. `exports["./plugin"]` stays as an alias.
- Add `exports["./adapter/tool-before"]`.
- `exports["./server"]` is not added: the host falls back to `main` (`resolvePackageEntrypoint`), and the live smoke proves it.

Live smoke CLI: `bun run smoke:opencode-entry` (needs the network on first run for the host's own SDK provisioning and the tarball's registry dependencies) runs `tooling/src/opencode-entry-smoke.ts`, with scenarios in `tooling/src/opencode-host/scenarios-entry.ts`. It reuses `resolveHostBinary`, `openSession`, `runHost` and `toolStates`, and exits 1 on any failed expectation.

## Failure modes and edge cases

| Input / state | Observable behavior | Propagation |
|---|---|---|
| `prepareEnforcement` throws (any cause) | Deny-all hook; `error` diagnostic with the message | Converted, never thrown to the host |
| Preflight fails (no git or Bun) | Deny-all: `toolu: not ready: toolu preflight: <reasons>` | Converted |
| No repo root (no option, no env, no bundled `plugins/`) | Deny-all naming `repoRoot` / `TOOLU_REPO_ROOT` | Converted |
| `options.repoRoot` present but not a non-empty string | Deny-all: `invalid plugin options: …`. The option is never silently ignored | Converted |
| `repoRoot` points at a missing tree | Selection fails → deny-all `toolu plugin selection: cannot read plugins root …` | Converted |
| Bootstrap NotReady (missing artifacts, bundle exit ≠ 0, timeout) | Deny-all with the bootstrap reason | Converted |
| Live smoke without network on first run | The host cannot provision its SDK or install the tarball's registry dependencies; the smoke exits 1 with the host error (never a pass) | Propagated |
| `client.app.log` rejects or hangs | Diagnostic dropped after at most 5 s; hooks unchanged | Recovered |
| Ready, unknown tool (`read`, `glob`, MCP, …) | Allowed by toolu (OP-03 extends coverage); user permission rules still apply | Skip |
| Ready, `bash`/`edit`/`write` with missing or non-string args | Throw `toolu: <tool> call missing …` | Denied |
| Gate returns `ask` | Throw with the gate reason (no host ask channel) | Denied until OP-05 |
| Dispatcher throws or returns malformed output | Throw `toolu dispatch: …` / runtime_failure reason | Denied |
| Worktree `/` (non-git project) | `projectRoot = directory` | Handled |
| Plugin listed both in config and as a local shim | The host loads both specs; only the first `server` call for the directory enforces, and the second returns `{}` with a `duplicate load skipped` diagnostic | Recovered |
| Instance disposed and re-created in one process (`opencode serve`) | `dispose` releases the directory, so the next init claims it again | Recovered |

## Acceptance criteria

- **AC-1:** Given a packed `@toolu/opencode` tarball (staged `plugins/` catalog, built like the published package) listed in `opencode.json` as the npm spec `@toolu/opencode@file:<tgz>`, with no options and no `TOOLU_REPO_ROOT`, the pinned `opencode-ai@1.18.34` host installs it through its npm route and loads the root export. The host emits exactly one `toolu: ready` diagnostic, a scripted `write` to a protected `.env` (`protectedFiles: block`) fails with the toolu gate reason, and the bytes of `.env` are unchanged.
- **AC-2:** Given a local shim `.opencode/plugins/toolu.ts` containing `export { default } from "@toolu/opencode";`, resolved through the project's `node_modules/@toolu/opencode` to the package root export, with `TOOLU_REPO_ROOT` in the environment, the pinned host emits exactly one `toolu: ready` diagnostic. A scripted protected `.env` write is denied with the bytes unchanged, and a scripted allowed bash `touch` creates its file (toolu allow passes through).
- **AC-3:** Given the config route `plugin: [["file://…/tools/toolu-opencode", { repoRoot: "<sandbox>/missing" }]]` (a package directory resolved through `main`), the pinned host emits exactly one `toolu: not ready` diagnostic. A scripted bash `touch` fails with a `toolu: not ready` error, and the file is never created.
- **AC-7:** Given both the npm spec of AC-1 in `opencode.json` and the local shim of AC-2 in one project, the pinned host emits exactly one `toolu: ready` diagnostic and one `toolu: duplicate load skipped` diagnostic. A scripted protected `.env` write is denied once, with the bytes unchanged.
- **AC-4:** `tsc --noEmit` typechecks `src/plugin/*.ts` and `src/adapter/tool-before.ts` against the pinned `@opencode-ai/plugin@1.18.34` declarations. There are no `as` casts, `any` or `@ts-expect-error` in those files, no handwritten replacement SDK types, and no remaining import of `@opencode/plugin`. A unit test scans those sources for ` as ` (other than `as const`), `: any`, `<any>`, `@ts-expect-error`, `@ts-ignore` and `@opencode/plugin"`.
- **AC-5:** In a hermetic test, `createTooluHooks` returns a `tool.execute.before` that throws `toolu: not ready` for every tool when given these bindings:
  - one whose preparation throws;
  - one with invalid `options.repoRoot`;
  - one with a missing repo root.

  It also records one `error` diagnostic each time. When given a ready binding over a real temp project and the repo's real plugins, it denies a protected `.env` edit and lets `bash: echo ok` through.
- **AC-6:** The root module's runtime exports are exactly `["default"]`. `default` is `{ id: "toolu", server: function }`. `mapToolCall` maps the real OpenCode arg shapes (`bash {command, description}`, `edit {filePath, oldString, newString}`, `write {filePath, content}`) to core requests, skips other tools, and denies gated tools with invalid args.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Packed tarball (temp copy of the package + `stagePlugins`, `bun pm pack --ignore-scripts`); isolated profile; git project with `.env`, `.opencode/toolu.config.json` block, `.opencode/toolu/plugins.json` `["toolu"]`; scripted provider `write` | One `toolu: ready` in `--print-logs` stderr; tool error contains the protected-files reason; `.env` bytes equal | npm route, options `undefined`, repo root from the bundled catalog; `@toolu/core` resolved from the registry as for real users | `bun run smoke:opencode-entry` (`entry.npm-root`) |
| AC-2 | Same project + `node_modules/@toolu/opencode` symlink + shim file; scripted `write .env` then `bash touch allowed.txt` | One ready line; `.env` unchanged; `allowed.txt` exists | Options `undefined` (local file route) | `bun run smoke:opencode-entry` (`entry.local-shim`) |
| AC-3 | Config route to the package directory, `repoRoot: <sandbox>/missing`; scripted bash `touch` | One not-ready line; bash error contains `toolu: not ready`; marker absent | Init failure must not fail open; options delivered through the config tuple | `bun run smoke:opencode-entry` (`entry.init-failure`) |
| AC-7 | AC-1 tarball spec + AC-2 shim and symlink in the same project | One ready line, one duplicate line; `.env` unchanged | Two module copies share the process-wide guard | `bun run smoke:opencode-entry` (`entry.both-routes`); hermetic `claimInstance`/`releaseInstance` test |
| AC-4 | The source files | Typecheck passes; no cast tokens | `any` from SDK `args` handled via `unknown` + Zod | `bun run typecheck`; `src/plugin/__tests__/toolu.test.ts` source scan |
| AC-5 | Temp project + real `plugins/` tree + real bootstrap | Throws/denies as stated; diagnostics recorded | Thrown preparation, invalid options, missing root | `bun test tools/toolu-opencode/src/plugin/__tests__` |
| AC-6 | Real module import; real arg shapes captured by the OP-01 probes | Exports and mappings as stated | Non-string `filePath`, missing `command`, unknown tool | `bun test tools/toolu-opencode/src/plugin/__tests__ tools/toolu-opencode/src/adapter/__tests__` |

The full gate is `bun run test`, which includes `check-workspace` (`default.id`), `test:pack`, conformance and `check:opencode-host`.

## Documentation impact

- `docs/opencode-host-contract.md`: the status paragraph says the shipped entry now conforms (OP-02), and lists the remaining owners.
- `docs/portable-core.md`:
  - the entry paragraph and the `@toolu/opencode/plugin` export row describe the `PluginModule`, `tool.execute.before` and the deny-all-on-init-failure rule;
  - a new `@toolu/opencode/adapter/tool-before` row is added;
  - the evaluate rows are marked as legacy-shaped, retained for the conformance suites.
- `docs/opencode.md`:
  - the prerequisite pins become `opencode-ai@1.18.34` and `@opencode-ai/plugin@1.18.34`;
  - the wiring `package.json` loses `@opencode/plugin`;
  - the entry description says `tool.execute.before`;
  - a note covers the `--print-logs` readiness diagnostic and the live smoke.

  The full rewrite stays with OP-29.
- `docs/conformance-report.md`: the SDK pin row notes that the `permission-evaluate` suite drives the retained evaluate-shaped adapter, not the host entry.
- `AGENTS.md` key files: none. The entry path is unchanged.

## Open Questions

None blocking. These were decided here, with Jev-assisted choices recorded in the brainstorm:

- export shape: default `PluginModule`;
- `ask` → deny until OP-05;
- keep the V2 evaluate adapter for its consumers;
- the live evidence is an opt-in script, not part of `bun run test`, until OP-28.

## Spec review

Jev was asked whether the ACs cover each issue acceptance item. The first answers were item 1 0.38, item 2 0.94 and item 3 0.93. After the fixes below, item 1 scored 0.81. Scope fit scored 0.90 and failure clarity 0.84.

- Acceptance criteria: 🔴 blocker (resolved): item 1's root npm export was exercised only through a package directory. AC-1 now installs a packed tarball through the host's npm route (`name@file:<tgz>`, verified on the pinned host with a scratch package).
- Acceptance criteria: 🔴 blocker (resolved): "exactly once" was unproven when both routes are configured. The architecture now has a once-per-directory guard, and AC-7 covers it.
- Acceptance criteria: 🟡 should-fix (resolved): AC-4's cast check was vague. It now names the exact forbidden tokens and the test that scans for them.
- Failure modes: 🟡 should-fix (resolved): offline first run of the live smoke had no stated behavior. Added: it exits 1 with the host error and never passes.
- Non-goals: 🔵 consider: keeping the evaluate-shaped adapter leaves two adapter shapes until OP-03/04/05. This is accepted because the shared `createGateDecider` prevents a duplicate gate list.
