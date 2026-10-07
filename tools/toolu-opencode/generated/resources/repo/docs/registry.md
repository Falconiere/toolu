# Hook module registry

Domain plugins (ast-grep and the language-quality plugins) add rules to toolu's core `PreToolUse` and `PostToolUse` dispatchers through a runtime registry in the host's config directory. `@toolu/core/registry` ([#257](https://github.com/Falconiere/toolu/issues/257)) imports the bundled ESM contributions in process. Both dispatchers and all shipped built-in gates are native TypeScript. ts-quality (#265), python-quality (#266), rust-quality (#267) and ast-grep (#268) are bundled registry modules.

## Layout

```
<config root>/toolu/pre-tools.d/<spec>__<name>.js    # tool/pre, also shell/pre
<config root>/toolu/post-tools.d/<spec>__<name>.js   # tool/post
```

`<config root>` is `TOOLU_CONFIG_DIR`, else the host's own root (`CLAUDE_CONFIG_DIR` or `~/.claude`, `CODEX_HOME` or `~/.codex`, …). `<spec>` is the owning plugin's `name@marketplace`. It may contain `.` but never `__`, `/` or whitespace.

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

`registerModules` copies each bundle to its target through `<target>.tmp.<pid>` and `rename`, and only when the bytes differ. It then removes every other `<spec>__*.js` and `<spec>__*.sh` in both directories: a plugin's pre-port `.sh` disappears the first time its TypeScript version registers. It also removes `<spec>__*.{js,sh}.tmp.*` residue older than a minute. Other plugins' entries are never touched. A missing bundle keeps the existing registry copy (stale enforcement beats none). The hook drains stdin, prints nothing on stdout, reports each failure on one stderr line and exits 0.

When `TOOLU_STARTUP_REPORT` names a file, the hook also appends one JSON record per declared module. The record carries `spec`, `name`, `event`, `source`, `target` and `status` (`written`, `unchanged` or `failed`, with `error`). It appends one `error` record for any other failure. `publishWrapper` does the same for each helper. Only the OpenCode bootstrap sets the variable ([#342](https://github.com/Falconiere/toolu/issues/342)): its readiness counts only what this run reported and verified on disk. A record that cannot be written makes the entry exit 1. With the variable unset, as on Claude Code and Codex, nothing is written.

On Codex, `pruneInactiveModules` removes modules of plugins that the ready plugin snapshot lists as absent. It never removes a symlink, and a missing or stale snapshot prunes nothing. The core SessionStart hook runs it once per session; dispatch-time gating also skips an absent plugin's modules.

## Dispatching

`runRegistry(event, ctx, options)` reads `<ctx.configRoot>/toolu/<dir>.d` and returns one outcome per module it reached, in byte order of file names. Shipped plugins register only `.js` modules:

| Situation | Outcome |
|---|---|
| Plugin definitively not installed (Claude `installed_plugins.json`, Codex snapshot) | `skipped: inactive`. An unreadable record fails open, and Cursor, Hermes and OpenCode count as installed |
| `.js` module | imported (fresh when its bytes change), checked against its file name, `run`, decision validated: `decision` |
| Import error, throw, rejection, contract mismatch, non-decision | `error`, plus `toolu-registry: module <file> failed: …; output skipped` on stderr; the walk continues |
| `deny` on a pre-tool event, `post_block` on a post-tool event | recorded, then the walk stops |
| Invalid module file name | never run; one stderr warning |

Merging outcomes (deny over ask over advisory) and encoding them for the host belong to the dispatcher.

## PreToolUse dispatch

toolu's PreToolUse entry for edit, shell and search tools runs the Bun bundle `hooks/dist/pre-tools.js`, wired with the generated launcher. The `mcp__` entry runs `hooks/dist/mcp-tools.js` the same way (#260). That bundle is `@toolu/core/gates/mcp-hook`: mcp-blocker alone, without the other built-ins or `pre-tools.d`, and it loads the gate only when a blocklist file or a toolu config exists. The subagent entry runs `hooks/dist/agent-tier.js` through its generated launcher (#262). `dispatchPreTool` (`@toolu/core/dispatch`) ports `pre-tools/mod.sh` and the PreToolUse half of `dispatch.sh`:

- **Order.** Built-in modules run in table order, then `runRegistry` walks `pre-tools.d` in file-name order. Bundled `.js` modules run in process.
- **Decisions.** The first deny is emitted exactly as its module wrote it, and the walk stops. A module exit of 2 blocks with that module's stderr. Any other non-zero exit is reported on stderr and skipped. The first ask is held and receives every advisory. Advisories are deduped and merged into one `additionalContext` and one `systemMessage`.
- **Edits.** Edit, Write, MultiEdit and `apply_patch` are walked once per affected path as a synthetic `Edit`. A deny or exit 2 on any path wins for the whole patch. Unparseable patch headers are denied.
- **Output.** Registry module results retain raw hook text. Native gates use the host encoder, which can print compact JSON where Bash's `jq -n` printed it pretty. Golden replay compares parsed decisions, exit codes, stderr and written state.
- **Built-ins.** All nine entries in `NATIVE_MODULES` (`plugins/toolu/hooks/src/pre-tools/builtins.ts`) are native `{ kind: "native", name, run(event, ctx) }` gates from `@toolu/core/gates`. `encodeDecision` degrades an `ask` on Codex to a deny where appropriate. The A, B and C golden suites replay captures of their Bash predecessors. `pre-tools-parity.test.ts` exercises the full conformance corpus on Claude and Codex, including registry and dispatcher cases.
- **Failure.** An unexpected dispatcher error exits 2 and blocks the tool, as a missing Bun does.

`bun run tooling/src/benchmarks/final-hook-latency.ts --runs N` compares the committed pre-tool and MCP bundles with the Bash versions extracted from tag `v7.2.0`. Samples run in back-to-back pairs. The 5 ms incremental cold-start budget is enforced by `bun run bench:shell --assert` on macOS arm64 (or with `TOOLU_LATENCY_ENFORCE=1`); Linux reports that result without failing the gate. See [the final conformance report](conformance-report.md) for current macOS and Linux measurements.

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
- **Environment.** Native and registry modules receive typed values: `event.toolName`, `ctx.raw` (the payload), `ctx.projectRoot`, `ctx.cwd`, `ctx.edit` and `ctx.configRoot`. The project root is the git toplevel of the hook's working directory, else that directory.
- **Built-ins.** `gate-status` and `push-waiver` are native (`@toolu/core/gates`), listed in `plugins/toolu/hooks/src/post-tools/builtins.ts`.
  - gate-status reads the command with `@toolu/core/shell`. It counts only a quality command the line runs, and only when a zero exit of the line would prove that command passed (`exitProves`). A pass needs every such command proven, a failure at least one. This fixes #283 items 6 and 7. `plugins/toolu/hooks/docs/gates.md` has the rules.
  - push-waiver detects pushes through the same layer (#283 item 8).

  Historical Bash versions are available at tag `v7.2.0` for parity checks.
- **Registry.** `post-tools.d` runs after the built-ins. ts-quality (#265), python-quality (#266) and rust-quality (#267) run there in process.
- **Parity.**
  - `plugins/toolu/hooks/src/__tests__/post-tools-283.test.ts` and the colocated native tests exercise the PostToolUse corpus on Claude Code and Codex. Historical Bash behavior remains available from tag `v7.2.0`; each language-quality plugin retains its captured golden suite (see below).
  - `post-tools-283.test.ts` pins the named #283 fixtures: there the bundle is right and bash is recorded as the known-wrong baseline.
- **Failure.** An unexpected dispatcher error exits 2 with `toolu PostToolUse dispatcher failed: <message>`, so the model sees that the post-tool checks did not run.

`bun run tooling/src/benchmarks/post-tools-latency.ts --runs N` measures the committed bundle against `bash mod.sh` from tag `v7.2.0`. Each variant runs in its own identically prepared sandbox, alternating runs to reduce load bias. The current figures are in [the final conformance report](conformance-report.md).

## Language-quality modules

A language-quality plugin is one `tool/post` module that checks the edited file and owns that file's entry in the quality gate. `@toolu/core/quality` (#265) holds what every such module shares, ported from the preamble and finalize fragments the bash modules repeated:

- `editedFile(event, ctx)`: `FILE_PATH` from `CLAUDE_FILE_PATHS`, else the Write/Edit/MultiEdit input (`path`, `file_path`, `target_file`), and whether a split patch deleted or moved it away.
- `fileQuality(event, ctx, spec)`: clears the entry of a removed file; skips a missing file, a file the module does not own (`spec.matches`) and, when asked, a file in a git linked worktree; then runs `spec.check` and settles the gate. Errors are recorded under the file's path with `spec.source` and `spec.reason`, and reported as `QUALITY VIOLATION — fix before proceeding:`. A clean file clears its entry and reports only the advisories.
- `astGrepScan(file, rules, ctx)`: one `ast-grep scan --inline-rules … --json` for every structural rule. A non-zero exit, stderr, or output that is not the JSON array is a failure with its stage, never "no hits".

ts-quality is the first such module: `plugins/ts-quality/hooks/src/post-tool-use.ts`, with one file of rules per family under `hooks/src/rules/`, in the order of the bash fragments they replace. Project checks come first, as in bash: a tracked `tsconfig*.json`, a lock file, and that package manager on `PATH`.

**Parity.** `hooks/src/__tests__/fixtures/golden.json` holds 120 captures, taken from the bash module at `a8b0c9c9`: every fixture of the ten concern bats suites, plus threshold sources (a `toolu.config.json` override, `max-lines` in `.eslintrc.json` and `.oxlintrc.json`, an unparsed eslint config, biome), Codex patches, deletes and moves, linked worktrees, jscpd and ast-grep failure stages. `golden-capture.ts` rebuilds that module from git objects, so the capture still runs now that the bash files are gone. `golden.test.ts` replays each case through the same post-tools bundle with the TypeScript module registered, and requires the same stdout, stderr, exit code and gate and telemetry files. Three things differ, and each has a test:

- With no `jq` on `PATH`, bash exited silently. The TypeScript module needs neither `jq` nor `TOOLU_LIB_DIR`, and checks the file (`DEV-1`).
- Bash listed no-mocks hits in ast-grep's output order, which varied from run to run. The module lists the first five in source order (`mock-order.test.ts`).
- If a regular file cannot be read, the module records a quality violation instead of treating the file as empty and clearing its gate entry (`read-failure.test.ts`).

**Latency.** `bun run tooling/src/benchmarks/ts-quality-latency.ts [--runs N] [--assert]` times the post-tools bundle running the bash module (from `a8b0c9c9`) against the same bundle running the TypeScript module. It uses two identical sandboxes and alternates the runs. On an Apple M2 Max with Bun 1.4.2 (7 runs, load average 5 to 9), the TypeScript module was 144 to 230 ms faster on every fixture:

| Fixture | bash module p50 | TS module p50 |
|---|---|---|
| three violations in fragment order | 350.8 ms | 144.5 ms |
| empty catch block (ast-grep) | 374.8 ms | 144.4 ms |
| `vi.mock` in a test file | 381.3 ms | 158.7 ms |
| long class method | 358.7 ms | 153.9 ms |
| clean file | 291.2 ms | 147.7 ms |

python-quality (#266) is the second: `plugins/python-quality/hooks/src/post-tool-use.ts`, with its rules under `hooks/src/rules/` in the order of the bash fragments: file size, test layout, suppression, function size, no-mocks, then the docstring advisory. Project checks come first, as in bash: a top-level `pyproject.toml`, `setup.py`, `setup.cfg` or `requirements.txt`, and `python3` on `PATH`. Unlike ts-quality, it checks files in linked worktrees, because the bash module did.

**Parity.** `hooks/src/__tests__/fixtures/golden.json` holds 97 captures, taken from the bash module at `c50c6bd9`: every fixture of the seven concern bats suites, plus threshold sources, Codex patches, deletes and moves, a linked worktree, ast-grep failure stages, and the awk and grep quirks the port keeps (CRLF lines, a tab in a hit line, a one-line def that swallows the next, symlinked siblings, the five- and three-hit caps). `golden.test.ts` replays each case through the post-tools bundle with the TypeScript module registered and requires the same stdout, stderr, exit code and gate and telemetry files. Two things differ, and each has a test:

- With no `jq` on `PATH`, bash exited silently. The TypeScript module needs neither `jq` nor `TOOLU_LIB_DIR`, and checks the file (`DEV-1`).
- If a regular file cannot be read, the module records a quality violation instead of treating the file as empty and clearing its gate entry (`read-failure.test.ts`).

The shared PostToolUse corpus and the language-quality golden suites cover multi-path patches using bundled registry modules.

**Latency.** `bun run tooling/src/benchmarks/python-quality-latency.ts [--runs N] [--assert]` times the bash module (from `c50c6bd9`) against the TypeScript module, as ts-quality's benchmark does (both use `tooling/src/benchmarks/quality-latency.ts`). On an Apple M2 Max with Bun 1.4.2 (7 runs, load average 14 to 24, other agents active), the TypeScript module was 53 to 160 ms faster on every fixture:

| Fixture | bash module p50 | TS module p50 |
|---|---|---|
| two violations in fragment order | 243.6 ms | 158.6 ms |
| bare `except:` | 279.5 ms | 119.6 ms |
| mock import in a test file (ast-grep) | 264.7 ms | 111.3 ms |
| def at the fn-length limit | 146.0 ms | 88.4 ms |
| clean file | 144.3 ms | 90.7 ms |

rust-quality (#267) is the third: `plugins/rust-quality/hooks/src/post-tool-use.ts`, with its rules under `hooks/src/rules/` in the order of the bash fragments. It runs in a project with `Cargo.toml` at the toplevel and `cargo` on `PATH`, checks `.rs` files including those in linked worktrees, and reads `rust-unsafe-exemptions.txt` from the settings directory through `@toolu/core/config`.

**Parity.** `plugins/rust-quality/hooks/src/__tests__/fixtures/golden.json` holds 120 captures from the bash module at `c50c6bd9`: every fixture of the nine concern bats suites, plus gating, Codex patches, deletes and moves, `CLAUDE_FILE_PATHS`, linked worktrees, the clippy hint, the exemption list, and ast-grep crash, non-JSON, empty and absent. `golden-corpus.test.ts` requires every bats test at the base, whose titles the capture records, to map to a case, and `golden.test.ts` replays each case with the TypeScript module and requires identical stdout, stderr, exit code and gate and telemetry files. Hits are filtered per rule, which keeps ast-grep's order stable, so no case needs reordering. Two things differ, each with a test: with no `jq` on `PATH` the module still checks the file (`DEV-1`), and an unreadable file is a violation (`read-failure.test.ts`).

**Latency.** `bun run tooling/src/benchmarks/rust-quality-latency.ts [--runs N] [--assert]` makes the same comparison against the bash module from `c50c6bd9`. On an Apple M2 Max with Bun 1.4.2 (7 runs, load average 14 to 16), the TypeScript module was 169 to 226 ms faster on every fixture:

| Fixture | bash module p50 | TS module p50 |
|---|---|---|
| two violations in rule order | 361.4 ms | 139.6 ms |
| `.unwrap()` in `src/` (ast-grep) | 347.4 ms | 131.2 ms |
| `#[automock]` in `src/` (ast-grep) | 354.4 ms | 128.5 ms |
| long method inside an impl | 315.3 ms | 135.2 ms |
| clean file | 288.0 ms | 119.1 ms |

## Native engine (Rust)

`toolu_engine` (`crates/core/engine`, #418) runs the same walk natively, and `toolu hook pre-tools` and `toolu hook post-tools` run it. `hooks.json` keeps the Bun bundles until #425, and the built-in gate tables stay empty until #419–#423 port the gates. `fixtures/dispatch/cases.json` holds the shared cases: the TypeScript dispatcher and the engine must both reproduce them byte for byte.

**Walk.** The built-ins run first, in table order, and each is folded as it decides. Then the registry phase runs every entry of the event directory in byte order of file names, until one stops the walk. Only after that are its results folded, as `runRegistry` and `walkRegistry` do. So stderr carries the built-in `exited` lines, then the `toolu-registry:` lines in walk order, then the registry `exited` lines. A module's own stderr is kept only when it exits 2.

**Manifests.** `<spec>__<name>.json` enables the rule of that spec and name compiled into the binary. The manifest format is `{"version":1,"spec","name","event","matcher"}` (`toolu_runtime::registry::manifest`). The matcher is tested against the walk's tool name, which is `Edit` for every path of a split edit. The rule then runs only if its `applies` holds. Nothing else costs anything: a `Read` with no fitting manifest calls no rule code and spawns no process. A manifest that cannot be read, has an unknown field, has another `version`, or (when its matcher fits the tool) names a rule the binary lacks prints one `toolu-registry: manifest <file> skipped: <reason>` line and is never an error. A usable manifest shadows its spec's `.js` and `.sh` modules, as a `.js` module shadows its spec's `.sh` one.

**Executables.** A `.sh` module keeps the TypeScript contract:
- it runs as `bash <path>` in the hook's cwd, with the payload plus a newline on stdin;
- its environment adds `input`, `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and `TOOLU_EDIT_*`;
- exit 2 is a hard deny, and any other status is skipped with `toolu-dispatch: module <file> exited <status>; output skipped`.

What is new is a deadline of 30 s per module. Past it, the module's process group is killed and it is reported as `exited 124`, even if it exits 2 while being killed. A crash reports 128 plus the signal. A process the module backgrounds with its streams redirected is not waited for. Output past 8 MiB is cut off: a deny still denies, and anything else is skipped with `printed more than 8388608 bytes`.

**Bun bridge.** A `.js` module runs through a temporary bridge, which #440 removes. A run of consecutive `.js` modules shares one `bun --no-install -e` process. That process imports each module in order, checks the default export and the decision as `runContract` and `DecisionSchema` do (with the same failure messages), and stops after a deny before a tool or a block after it. A batch's deadline is 30 s per module in it. A module that hangs, exits the process or garbles its line fails alone (`timed out after <ms> ms`, `bridge exited <status>`, `bridge output unreadable`), and the modules after it run in a fresh process. A module that hangs first in a batch of four therefore holds the other three for the batch's whole deadline before they re-run. The hook timeout in `hooks.json` (#425) bounds that. Bun is looked for in `TOOLU_BUN`, then on `PATH`, then in `~/.bun/bin/bun`. Without it, the first applicable pre-tool `.js` module is a deny that names the module and the install. After a tool, the modules are skipped and one advisory per session says which ones did not run; a marker under `<project>/<host dir>/tmp/registry-bridge/` (`.claude`, `.codex`, …) records that.

**Deviations from the Bun dispatcher.**
- The `toolu` CLI writes the dispatcher's stderr with a final newline. A module stderr that lacks one gains it, and a stderr of a lone newline prints nothing. The engine's own result is exact.
- A payload that is not UTF-8 fails closed (exit 2), where Bun decoded it lossily.
- A lone surrogate escape reads as U+FFFD. So the synthetic `Edit` payload of a split patch re-prints that character, where TypeScript re-prints the escape.

**Payload.** The payload is parsed as `JSON.parse` would read it. Lone surrogate escapes read as U+FFFD, so gates still see the tool, while modules get the raw text. A payload nested past serde's limit (128 levels) is denied before a tool and blocked after it, because the engine cannot read what the gates would check.

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
- **byte-savings** (`tool/post`). Appends `{"kind","returned","full"}` to `<config>/toolu/byte-savings/<session>.jsonl` for Read, Grep, Glob and a Bash or Shell line that runs `ast-grep` or `sg`, parsed the same way (wrappers such as `timeout` included). `returned` is measured exactly as the bash module's jq did (`.tool_response` text, else its `content`, `stdout` or `output`). OpenCode's post bridge puts the host's result text in `tool_response.output`. On OpenCode only, an ast-grep record also returns an advisory holding the session's report (`lib/savings-report.ts`, shared with `byte-savings-report.js`); every other host gets `allow`.

Both bundles inline unbash and zod, about 310 KB each; the latency numbers below include parsing them.

**Parity.** `hooks/src/__tests__/fixtures/golden.json` holds captures of the bash plugin at `2912cd9d`, run through toolu's committed pre-tools and post-tools bundles: 41 search-nudge case/host pairs, 36 byte-savings cases with their ledgers, 6 report runs, and the 2 #258 corpus fixtures search-nudge decides, on both hosts. `golden-capture.ts` rebuilds the bash plugin from git objects, so the capture still runs now that its files are gone. `golden-nudge.test.ts`, `golden-savings.test.ts`, `golden-corpus.test.ts` and `byte-savings-report.test.ts` replay every case with the TypeScript modules registered and require the same stdout, stderr, exit code and ledgers. Those two fixtures stay in the shared `@toolu/conformance` PreToolUse corpus, where `pre-tools-parity.test.ts` checks their decision class. The named deviations are the #283 item 10 defects of the bash text match:

- a grep filtering a pipe, and grep or rg inside a commit message, no longer nudge;
- a `for … in` loop gets the generic nudge, not STOP;
- a structural pattern passed as one quoted argument (`rg -n "fn main" src/`, `grep -r "impl Foo" src`, `find … -exec grep -n 'fn main' {} +`) gets STOP, not the generic nudge;
- `echo grep me` and `echo "run ast-grep later"` are not searches; `timeout 60 sg run …` is recorded;
- a grep in a heredoc that bash runs (`bash <<'EOF'`) gets the generic nudge; bash stripped every heredoc body and stayed silent.

Commands the shared parser leaves as arguments are still found, as bash's text match found them: the program after `find -exec`/`-execdir`/`-ok`/`-okdir`, and the first non-flag argument after `watch`, `npx`, `bunx`, `pnpx`, `pnpm exec|dlx`, `yarn exec|dlx`, `npm exec` and `bun x` (`lib/launched.ts`).

The report CLI's usage line names `byte-savings-report.js`. An empty ledger and a line that is not a ledger record are not in the capture, because bash printed a jq error there. `byte-savings-report.test.ts` pins what the Bun CLI prints instead: `TOTAL returned: 0 bytes (~0 tok)`, and `byte-savings-report: <file>:<n>: invalid ledger line` with exit 1.

The skill wrapper `hooks/dist/ast-grep.js` is checked against the live ast-grep CLI given the argv the bash wrapper built (`ast-grep-cli.test.ts`), because ast-grep's output changes between versions.

**Latency.** `bun run tooling/src/benchmarks/ast-grep-latency.ts [--runs N] [--assert]` times toolu's bundles running the bash modules (from `2912cd9d`) against the same bundles running the TypeScript modules, in two identical sandboxes with alternating runs. On an Apple M2 Max with Bun 1.4.2 (25 runs, load average 5 to 8, other agents active), the TypeScript modules were faster on every fixture. With 9 runs at load 10 to 26, single spikes of several hundred milliseconds moved the Bash structural grep p50 either way, so measure with at least 25:

| Fixture | bash module p50 | TS module p50 |
|---|---|---|
| Bash structural grep | 158.2 ms | 126.3 ms |
| Grep tool, structural pattern | 135.4 ms | 93.4 ms |
| Bash `ls -la` (silent) | 136.6 ms | 129.2 ms |
| PostToolUse, Bash `ast-grep run` | 85.6 ms | 71.3 ms |
| PostToolUse, plain Bash (ignored) | 77.5 ms | 74.3 ms |
