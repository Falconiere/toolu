# Hook module registry

Domain plugins (ast-grep and the language-quality plugins) add rules to toolu's core `PreToolUse` and `PostToolUse` dispatchers through a runtime registry in the host's config directory. `@toolu/core/registry` ([#257](https://github.com/Falconiere/toolu/issues/257)) is the TypeScript version of it: each contribution is one bundled ESM module, and the dispatcher imports it in-process instead of spawning `bash` per module. PreToolUse runs through the TypeScript dispatcher (#258), which runs `.sh` modules through a bash fallback. PostToolUse keeps the bash dispatcher until #259, and each plugin keeps its `.sh` modules until it is ported (#265 to #268).

## Layout

```
<config root>/toolu/pre-tools.d/<spec>__<name>.js    # tool/pre, also shell/pre
<config root>/toolu/post-tools.d/<spec>__<name>.js   # tool/post
```

`<config root>` is `TOOLU_CONFIG_DIR`, else the host's own root (`CLAUDE_CONFIG_DIR` or `~/.claude`, `CODEX_HOME` or `~/.codex`, …). `<spec>` is the owning plugin's `name@marketplace`. It may contain `.` but never `__`, `/` or whitespace. The directories are the same ones the bash `*.sh` modules use.

## Module contract

A module's default export:

```ts
import { defineRegistryModule } from "@toolu/core/registry";

export default defineRegistryModule({
  spec: "ts-quality@toolu",   // must match the file name
  name: "ts-quality",          // must match the file name
  event: "tool/post",          // must match the directory
  async run(event, ctx) {      // event: the NormalizedEvent (tool/pre, shell/pre or tool/post)
    return { kind: "allow" };  // a Decision from @toolu/core/decision
  },
});
```

`ctx` carries `host`, `env`, `configRoot`, `projectRoot`, the host's `raw` payload and, for one path of a split multi-file patch, `edit`. It is an interface, so later layers add optional fields (the parsed shell command from #284) without breaking modules. A module owns its side effects: a quality module records or clears its own gate entry, once per event, as its bash predecessor did.

## Registering

The plugin's SessionStart entry is a bundle of its own (`hooks/src/register.ts` → `hooks/dist/register.js`, wired with the generated launcher):

```ts
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("ts-quality@toolu", [
  { name: "ts-quality", event: "tool/post", bundle: join(import.meta.dir, "post-tool-use.js") },
]);
```

`registerModules` copies each bundle to its target through `<target>.tmp.<pid>` and `rename`, and only when the bytes differ. It then removes every other `<spec>__*.js` and `<spec>__*.sh` in both directories: a plugin's pre-port `.sh` disappears the first time its TypeScript version registers. It also removes `<spec>__*.{js,sh}.tmp.*` residue older than a minute. Other plugins' entries are never touched. A missing bundle keeps the existing registry copy (stale enforcement beats none). The hook drains stdin, prints nothing on stdout, reports each failure on one stderr line and always exits 0.

On Codex, `pruneInactiveModules` removes `.js` and `.sh` modules of plugins that the ready plugin snapshot lists as absent. It never removes a symlink, and a missing or stale snapshot prunes nothing. The core SessionStart hook runs it once per session once #263 lands; until then the bash prune covers `.sh` modules, and dispatch-time gating skips an absent plugin's `.js` modules.

## Dispatching

`runRegistry(event, ctx, { fallback?, warn? })` reads `<ctx.configRoot>/toolu/<dir>.d` and returns one outcome per module it reached, in byte order of file names (bash glob order under `LC_ALL=C`):

| Situation | Outcome |
|---|---|
| Plugin definitively not installed (Claude `installed_plugins.json`, Codex snapshot) | `skipped: inactive`. An unreadable record fails open, and Cursor, Hermes and OpenCode count as installed |
| `.sh` module whose spec also has a `.js` module | `skipped: shadowed`, so one plugin never writes the gate twice per event |
| Other `.sh` module | passed to `fallback` (the bash bridge, #258), else `skipped: bash` |
| `.js` module | imported (fresh when its bytes change), checked against its file name, `run`, decision validated: `decision` |
| Import error, throw, rejection, contract mismatch, non-decision | `error`, plus `toolu-registry: module <file> failed: …; output skipped` on stderr; the walk continues |
| `deny` on a pre-tool event, `post_block` on a post-tool event | recorded, then the walk stops, as in `dispatch.sh` |
| File not named `<spec>__<name>.{js,sh}` | never run; one stderr warning |

Merging outcomes (deny over ask over advisory) and encoding them for the host belong to the dispatcher.

## PreToolUse dispatch

toolu's PreToolUse entry for edit, shell and search tools runs the Bun bundle `hooks/dist/pre-tools.js`, wired with the generated launcher. The standalone `mcp__` and subagent entries keep running `mcp-blocker.sh` and `agent-tier.sh` directly: a Bun wrapper around an unported script would add Bun's startup, about 20 ms, to every call. #260 and #262 switch each entry when its module is ported. `dispatchPreTool` (`@toolu/core/dispatch`) is a TypeScript port of `pre-tools/mod.sh` and `dispatch.sh`:

- **Order.** Built-in modules run in table order, which is the byte order `mod.sh` globbed. `runRegistry` then walks `pre-tools.d`. `.js` modules run in process. `.sh` modules run on bash with the environment `mod.sh` exported: `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR`, and `TOOLU_EDIT_*` during a patch walk.
- **Decisions.** The first deny is emitted exactly as its module wrote it, and the walk stops. A module exit of 2 blocks with that module's stderr. Any other non-zero exit is reported on stderr and skipped. The first ask is held and receives every advisory. Advisories are deduped and merged into one `additionalContext` and one `systemMessage`.
- **Edits.** Edit, Write, MultiEdit and `apply_patch` are walked once per affected path as a synthetic `Edit`. A deny or exit 2 on any path wins for the whole patch. Unparseable patch headers are denied.
- **Output.** Module results stay raw hook text, and the merged object is printed the way `jq -n` prints it. While a module runs on bash, the bundle's stdout and exit code match `bash mod.sh` byte for byte. `plugins/toolu/hooks/src/__tests__/pre-tools-parity.test.ts` checks this over the `@toolu/conformance` PreToolUse corpus, as Claude Code and Codex deliver it.
- **Cutover.** A port replaces that module's `bashModule(...)` in `plugins/toolu/hooks/src/pre-tools/builtins.ts` with a native `{ kind: "native", name, run(event, ctx) }`, whose signature matches `RegistryModule.run`. Its `Decision` is encoded for the host with `encodeDecision`, so an `ask` on Codex becomes a deny.
- **Failure.** An unexpected dispatcher error exits 2 and blocks the tool, as a missing Bun does.

`bun run tooling/src/benchmarks/pre-tools-latency.ts [--runs N] [--assert]` measures p50 for `bash mod.sh`, the bundle, and the bundle with one module native, against the epic budget of bash + 5 ms. On an Apple M2 Max with Bun 1.4.2, the bundle ran 40 to 182 ms faster than `bash mod.sh` on every fixture: it drops the dispatcher's own `jq` calls per module. With one module native, six of the seven fixtures ran a further 13 to 39 ms faster, and one was 11 ms slower.

## PostToolUse dispatch

toolu's PostToolUse entry for edit, shell and search tools runs the Bun bundle `hooks/dist/post-tools.js`, wired with the generated launcher. With no Bun it prints a `systemMessage` and exits 0, because PostToolUse is not an enforcing event. `dispatchPostTool` (`@toolu/core/dispatch`) ports `post-tools/mod.sh` and the PostToolUse half of `dispatch.sh`. It shares the PreToolUse walk, with these differences:

- **Decisions.** The first `decision: "block"` is emitted exactly as its module wrote it, and the walk stops. `permissionDecision` means nothing after the tool ran: no ask is held and no deny stops the walk. Exit codes and advisory merging are as in PreToolUse, and the merged object names `hookEventName: "PostToolUse"`.
- **Edits.** A block or exit 2 on any path of a patch wins for the whole patch. Unparseable `apply_patch` headers give `{"decision":"block","reason":"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run."}`.
- **Environment.** `.sh` modules also get `PROJECT_ROOT`, which is the git toplevel of the hook's working directory, else that directory. `$PROJECT_ROOT/node_modules/.bin` goes first on `PATH`. Native and `.js` modules receive the same values typed: `event.toolName`, `ctx.raw` (the payload), `ctx.projectRoot`, `ctx.cwd`, `ctx.edit` and `ctx.configRoot`.
- **Built-ins.** `gate-status` and `push-waiver` are native (`@toolu/core/gates`), listed in `plugins/toolu/hooks/src/post-tools/builtins.ts`.
  - gate-status reads the command with `@toolu/core/shell`. It records only a quality command the line runs whose exit status is the line's (`exitProves`), which fixes #283 items 6 and 7. `plugins/toolu/hooks/docs/gates.md` has the rules.
  - push-waiver detects pushes through the same layer (#283 item 8).
  
  Their bash scripts stay as the parity baseline.
- **Registry.** `post-tools.d` runs after the built-ins. The language-quality modules (#265–#267) still run there on bash.
- **Parity.**
  - `plugins/toolu/hooks/src/__tests__/post-tools-parity.test.ts` runs the `@toolu/conformance` PostToolUse corpus as Claude Code and Codex deliver it. It compares stdout, the exit code and the project's gate, waiver and telemetry files between `bash mod.sh` and the bundle. The corpus includes the three language-quality plugins registered by their real `register.sh`.
  - `post-tools-283.test.ts` pins the named #283 fixtures: there the bundle is right and bash is recorded as the known-wrong baseline.
- **Failure.** An unexpected dispatcher error exits 2 with `toolu PostToolUse dispatcher failed: <message>`, so the model sees that the post-tool checks did not run.

`bun run tooling/src/benchmarks/post-tools-latency.ts [--runs N] [--assert]` measures p50 for `bash mod.sh` against the bundle, each in its own identically prepared sandbox. On an Apple M2 Max with Bun 1.4.2 (9 runs, load average about 4), the bundle ran 29 to 177 ms faster on every fixture. The largest gain was a two-path patch through ts-quality and rust-quality: 747 ms against 571 ms.

## Import cost

Measured by `packages/toolu-core/src/registry/__tests__/registry-import-cost.test.ts` on an Apple M2 Max, macOS 26.6.2, Bun 1.4.2. The test runs 20 modules, each a separate 112.8 KB bundle that inlines the state layer and zod, through `runRegistry` in a fresh `bun` process, over 15 measured runs of the whole set.

| Measure | p50 | p95 |
|---|---|---|
| One module, import + validate + run, inside the process | 4.8 ms | 9.4 ms |
| Whole-process marginal cost per module (20 modules vs none) | 5.6 ms | — |
| One bash registry module under the bash dispatcher (one `bash` spawn), same run | 4.8 ms | — |

A bundled module costs about what the bash module it replaces cost. Most of it is parsing that bundle's own copies of `@toolu/core` and zod, so each registry module should import only the core entries it uses.
