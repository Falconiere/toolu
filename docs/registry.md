# Hook module registry

Domain plugins (ast-grep and the language-quality plugins) add rules to toolu's core `PreToolUse` and `PostToolUse` dispatchers through a runtime registry in the host's config directory. `@toolu/core/registry` ([#257](https://github.com/Falconiere/toolu/issues/257)) imports bundled ESM contributions in-process and runs remaining registry `.sh` modules through Bash. Both dispatchers and all toolu built-in gates are native TypeScript; domain plugins keep their `.sh` modules until they are ported (#265 to #268).

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

toolu's PreToolUse entry for edit, shell and search tools runs the Bun bundle `hooks/dist/pre-tools.js`, wired with the generated launcher. The `mcp__` entry runs `hooks/dist/mcp-tools.js` the same way (#260). That bundle is `@toolu/core/gates/mcp-hook`: mcp-blocker alone, without the other built-ins or `pre-tools.d`, and it loads the gate only when a blocklist file or a toolu config exists. The subagent entry runs `hooks/dist/agent-tier.js` through its generated launcher (#262). `dispatchPreTool` (`@toolu/core/dispatch`) ports `pre-tools/mod.sh` and the PreToolUse half of `dispatch.sh`:

- **Order.** Built-in modules run in table order, which is the byte order `mod.sh` globbed. `runRegistry` then walks `pre-tools.d`. `.js` modules run in process. `.sh` modules run on bash with the environment `mod.sh` exported: `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR`, and `TOOLU_EDIT_*` during a patch walk.
- **Decisions.** The first deny is emitted exactly as its module wrote it, and the walk stops. A module exit of 2 blocks with that module's stderr. Any other non-zero exit is reported on stderr and skipped. The first ask is held and receives every advisory. Advisories are deduped and merged into one `additionalContext` and one `systemMessage`.
- **Edits.** Edit, Write, MultiEdit and `apply_patch` are walked once per affected path as a synthetic `Edit`. A deny or exit 2 on any path wins for the whole patch. Unparseable patch headers are denied.
- **Output.** Registry module results retain raw hook text. Native gates use the host encoder, which can print compact JSON where Bash's `jq -n` printed it pretty. Golden replay compares parsed decisions, exit codes, stderr and written state.
- **Built-ins.** All nine entries in `NATIVE_MODULES` (`plugins/toolu/hooks/src/pre-tools/builtins.ts`) are native `{ kind: "native", name, run(event, ctx) }` gates from `@toolu/core/gates`. `encodeDecision` degrades an `ask` on Codex to a deny where appropriate. The A, B and C golden suites replay captures of their Bash predecessors. `pre-tools-parity.test.ts` exercises the full conformance corpus on Claude and Codex, including registry and dispatcher cases.
- **Failure.** An unexpected dispatcher error exits 2 and blocks the tool, as a missing Bun does.

`bun run tooling/src/benchmarks/pre-tools-latency.ts [--runs N] [--assert]` measures p50 against the epic budget of bash + 5 ms. It compares `bash mod.sh` and `bash mcp-blocker.sh`, extracted with `git archive 2386d4f3 plugins/toolu` (the last commit where every module ran on bash), with the two committed bundles. Samples run in back-to-back pairs.

On an Apple M2 Max with Bun 1.4.2, measured on 2026-09-29 with 40 pairs:

| Hook | bash p50 | bundle p50 | bundle − bash |
|---|---|---|---|
| Dispatcher, 7 fixtures | 265 to 979 ms | 149 to 860 ms | −97 to −430 ms |
| `mcp__`, blocked server | 47.6 ms | 32.6 ms | −15.0 ms |
| `mcp__`, unlisted server | 34.1 ms | 32.9 ms | −1.2 ms |
| `mcp__`, no blocklist and no config | 24.3 ms | 28.8 ms | +4.5 ms |

The machine was shared with other agent sessions (load average 7 to 11).

The last row is the tightest. That call only needs two file checks, and bash sources three libs to make them, while Bun pays its startup plus parsing the whole bundle. Parsing the lazily loaded gate code costs about 6 ms. Under heavier load (load average 12 and up), the two allowed `mcp__` rows measured 12 to 13 ms over bash, so re-measure on a quiet machine for the epic's latency evidence (#279).

## PostToolUse dispatch

toolu's PostToolUse entry for edit, shell and search tools runs the Bun bundle `hooks/dist/post-tools.js`, wired with the generated launcher. With no Bun it prints a `systemMessage` and exits 0, because PostToolUse is not an enforcing event. `dispatchPostTool` (`@toolu/core/dispatch`) ports `post-tools/mod.sh` and the PostToolUse half of `dispatch.sh`. It shares the PreToolUse walk, with these differences:

- **Decisions.** The first `decision: "block"` is emitted exactly as its module wrote it, and the walk stops. `permissionDecision` means nothing after the tool ran: no ask is held and no deny stops the walk. Exit codes and advisory merging are as in PreToolUse, and the merged object names `hookEventName: "PostToolUse"`.
- **Edits.** A block or exit 2 on any path of a patch wins for the whole patch. Unparseable `apply_patch` headers give `{"decision":"block","reason":"Unable to parse apply_patch file headers; per-file post-edit quality checks could not run."}`.
- **Environment.** `.sh` modules also get `PROJECT_ROOT`, which is the git toplevel of the hook's working directory, else that directory. `$PROJECT_ROOT/node_modules/.bin` goes first on `PATH`. Native and `.js` modules receive the same values typed: `event.toolName`, `ctx.raw` (the payload), `ctx.projectRoot`, `ctx.cwd`, `ctx.edit` and `ctx.configRoot`.
- **Built-ins.** `gate-status` and `push-waiver` are native (`@toolu/core/gates`), listed in `plugins/toolu/hooks/src/post-tools/builtins.ts`.
  - gate-status reads the command with `@toolu/core/shell`. It counts only a quality command the line runs, and only when a zero exit of the line would prove that command passed (`exitProves`). A pass needs every such command proven, a failure at least one. This fixes #283 items 6 and 7. `plugins/toolu/hooks/docs/gates.md` has the rules.
  - push-waiver detects pushes through the same layer (#283 item 8).
  
  Their bash scripts stay as the parity baseline.
- **Registry.** `post-tools.d` runs after the built-ins. The language-quality modules (#265–#267) still run there on bash.
- **Parity.**
  - `plugins/toolu/hooks/src/__tests__/post-tools-parity.test.ts` runs the `@toolu/conformance` PostToolUse corpus as Claude Code and Codex deliver it. It compares stdout, the exit code and the project's gate, waiver and telemetry files between `bash mod.sh` and the bundle. The corpus includes the three language-quality plugins registered by their real `register.sh`.
  - `post-tools-283.test.ts` pins the named #283 fixtures: there the bundle is right and bash is recorded as the known-wrong baseline.
- **Failure.** An unexpected dispatcher error exits 2 with `toolu PostToolUse dispatcher failed: <message>`, so the model sees that the post-tool checks did not run.

`bun run tooling/src/benchmarks/post-tools-latency.ts [--runs N] [--assert]` measures p50 for `bash mod.sh` against the bundle. Each variant runs in its own identically prepared sandbox, and the two alternate run by run, so a load change during a fixture hits both. On an Apple M2 Max with Bun 1.4.2 (9 runs, load average 5 to 8, other agents active), the bundle ran 50 to 184 ms faster on every fixture. The largest gain was a two-path patch through ts-quality and rust-quality: 891 ms against 708 ms. Before the runs were interleaved, one load spike put the bash block and the bundle block on different sides of it, and a fixture read 13 ms slower.

## Import cost

Measured by `packages/toolu-core/src/registry/__tests__/registry-import-cost.test.ts` on an Apple M2 Max, macOS 26.6.2, Bun 1.4.2. The test runs 20 modules, each a separate 112.8 KB bundle that inlines the state layer and zod, through `runRegistry` in a fresh `bun` process, over 15 measured runs of the whole set.

| Measure | p50 | p95 |
|---|---|---|
| One module, import + validate + run, inside the process | 4.8 ms | 9.4 ms |
| Whole-process marginal cost per module (20 modules vs none) | 5.6 ms | — |
| One bash registry module under the bash dispatcher (one `bash` spawn), same run | 4.8 ms | — |

A bundled module costs about what the bash module it replaces cost. Most of it is parsing that bundle's own copies of `@toolu/core` and zod, so each registry module should import only the core entries it uses.

## ast-grep modules

ast-grep (#268) registers two bundled modules from `plugins/ast-grep/hooks/src`, published by `hooks/dist/register.js` behind its hooks.json launcher:

- **search-nudge** (`tool/pre`). A Grep whose pattern looks structural (`fn `, `class `, `=>`, …) gets the STOP nudge unless its glob, type or path names non-code files. A Bash or Shell command is read with `@toolu/core/shell`: only a `grep`, `rg` or `git grep` the line actually runs counts, and a grep that filters a pipe (`… | grep x`, but not `… | xargs grep x`) is left alone. ast-grep's state (the `skills.ast-grep` opt-out, then `sg`/`ast-grep` on `PATH`) is looked up only once a nudge is due.
- **byte-savings** (`tool/post`). Appends `{"kind","returned","full"}` to `<config>/toolu/byte-savings/<session>.jsonl` for Read, Grep, Glob and a Bash or Shell line that runs `ast-grep` or `sg`, parsed the same way (wrappers such as `timeout` included). `returned` is measured exactly as the bash module's jq did.

Both bundles inline unbash and zod, about 310 KB each; the latency numbers below include parsing them.

**Parity.** `hooks/src/__tests__/fixtures/golden.json` holds captures of the bash plugin at `2912cd9d`, run through toolu's committed pre-tools and post-tools bundles: 39 search-nudge case/host pairs, 34 byte-savings cases with their ledgers, 6 report runs, and the 4 #258 corpus fixtures search-nudge decides. `golden-capture.ts` rebuilds the bash plugin from git objects, so the capture still runs now that its files are gone. `golden-nudge.test.ts`, `golden-savings.test.ts`, `golden-corpus.test.ts` and `byte-savings-report.test.ts` replay every case with the TypeScript modules registered and require the same stdout, stderr, exit code and ledgers. The corpus fixtures left `pre-tools-parity.test.ts`, because `bash mod.sh` cannot run a `.js` module. The named deviations are the #283 item 10 defects of the bash text match:

- a grep filtering a pipe, and grep or rg inside a commit message, no longer nudge;
- a `for … in` loop gets the generic nudge, not STOP;
- a structural pattern passed as one quoted argument (`rg -n "fn main" src/`, `grep -r "impl Foo" src`) gets STOP, not the generic nudge;
- `echo grep me` and `echo "run ast-grep later"` are not searches; `timeout 60 sg run …` is recorded.

The skill wrapper `hooks/dist/ast-grep.js` is checked against the live ast-grep CLI given the argv the bash wrapper built (`ast-grep-cli.test.ts`), because ast-grep's output changes between versions.

**Latency.** `bun run tooling/src/benchmarks/ast-grep-latency.ts [--runs N] [--assert]` times toolu's bundles running the bash modules (from `2912cd9d`) against the same bundles running the TypeScript modules, in two identical sandboxes with alternating runs. On an Apple M2 Max with Bun 1.4.2 (15 runs, load average 17 to 26, other agents active), the TypeScript modules were within budget on every fixture:

| Fixture | bash module p50 | TS module p50 |
|---|---|---|
| Bash structural grep | 799.3 ms | 698.6 ms |
| Grep tool, structural pattern | 250.2 ms | 136.5 ms |
| Bash `ls -la` (silent) | 142.0 ms | 142.5 ms |
| PostToolUse, Bash `ast-grep run` | 108.2 ms | 96.0 ms |
