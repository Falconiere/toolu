# Hook module registry

Domain plugins add rules to toolu's `PreToolUse` and `PostToolUse` hooks through a runtime registry in the host's config directory. The native engine (`toolu_engine`, `crates/core/engine`, #418) walks it inside `toolu hook pre-tools` and `toolu hook post-tools`. The first-party plugins (ast-grep, ts-quality, python-quality and rust-quality) register manifests that enable rules compiled into `toolu`. A third-party plugin registers an executable module.

## Layout

```
<config root>/toolu/pre-tools.d/<spec>__<name>.json   # a manifest: a compiled-in rule
<config root>/toolu/pre-tools.d/<spec>__<name>.sh     # an executable module
<config root>/toolu/post-tools.d/…                    # the same after a tool
```

`<config root>` is `TOOLU_CONFIG_DIR`, else the host's own root (`CLAUDE_CONFIG_DIR` or `~/.claude`, `CODEX_HOME` or `~/.codex`, …). `<spec>` is the owning plugin's `name@marketplace`. It may contain `.` but never `__`, `/` or whitespace. A file without the `<spec>__<name>` namespace is never run and gets one stderr warning per walk.

## Manifests

`<spec>__<name>.json` enables the rule of that spec and name compiled into the binary: `{"version":1,"spec","name","event","matcher"}` (`toolu_runtime::registry::manifest`). The matcher is tested against the walk's tool name, which is `Edit` for every path of a split edit, and the rule then runs only if its `applies` holds. Nothing else costs anything: a `Read` with no fitting manifest calls no rule code and spawns no process. A manifest that cannot be read, has an unknown field or another `version`, or (when its matcher fits the tool) names a rule the binary lacks prints one `toolu-registry: manifest <file> skipped: <reason>` line and is never an error. A usable manifest shadows its spec's `.sh` modules.

## Executable modules

A third-party rule is an executable module, `<spec>__<name>.sh`, unchanged from the TypeScript dispatcher it replaces:

- it runs as `bash <path>` in the hook's working directory, with the raw hook payload plus a newline on stdin;
- its environment adds `input` (the payload), `tool_name`, `TOOLU_LIB_DIR`, `TOOLU_CONFIG_DIR` and, for one path of a split multi-file patch, `TOOLU_EDIT_OPERATION`, `TOOLU_EDIT_FROM` and `TOOLU_EDIT_MOVED_TO`;
- it prints hook-format JSON on stdout (`hookSpecificOutput` with `permissionDecision`, `additionalContext`, `systemMessage`, or a PostToolUse `{"decision":"block"}`) or nothing;
- exit 2 is a hard deny, with the module's stderr as the reason; any other non-zero status is skipped with `toolu-dispatch: module <file> exited <status>; output skipped`.

A module has 30 s. Past that its process group is killed and it is reported as `exited 124`, even if it exits 2 while being killed. A crash reports 128 plus the signal. A process the module backgrounds with its streams redirected is not waited for. Output past 8 MiB is cut off: a deny still denies, and anything else is skipped with `printed more than 8388608 bytes`.

## JavaScript modules

Until #440 a `.js` module (a bundled ESM `defineRegistryModule` export) ran through a Bun bridge. toolu no longer runs JavaScript: a `<spec>__<name>.js` file is listed and gated like any module, but it is never run, never spawns anything, prints nothing per tool call, and no longer shadows its spec's `.sh` module. Its walk step is `Skipped(Unsupported)`. Instead, `toolu hook session-start` adds one line per `.js` file to the session's `systemMessage`:

```
toolu-registry: pre-tools.d/<spec>__<name>.js is a JavaScript module, which toolu no longer runs; publish it as an executable <spec>__<name>.sh (docs/registry.md#executable-modules)
```

`toolu doctor` reports the same files as a `registry` warning. A first-party plugin's register removes its own old `.js` module and tells the user once.

## Registering

A first-party plugin's native SessionStart `register` hook (`toolu <plugin> hook register`) writes its manifest atomically, only when the bytes differ. It then removes every other `<spec>__*.js` and `<spec>__*.sh` of that spec in both directories (the Bun-era module and `.sh` bridge), and aged `*.tmp` residue. Other plugins' entries are never touched. The quality plugins share this through `toolu_engine::quality::QualityPlugin`.

When `TOOLU_STARTUP_REPORT` names a file, the hook appends one JSON record per manifest (`spec`, `name`, `event`, `target`). A self-hosting bootstrap can set the variable ([#342](https://github.com/Falconiere/toolu/issues/342)) to count only what a run reported and verified on disk. With the variable unset, as on Claude Code and Codex, nothing is written.

On Codex, the core SessionStart hook prunes modules of plugins that the ready plugin snapshot lists as absent. It never removes a symlink, and a missing or stale snapshot prunes nothing. Walk-time gating also skips an absent plugin's modules.

## The walk

The built-ins run first, in table order, and each is folded as it decides. The pre-tool table has nine gates: bash-commands, code-edit-rules, commit-gate, docs-sync, mcp-blocker, plan-ledger, protected-files, push-review and quality-gate (#419–#422). The post-tool table holds gate-status and push-waiver (#423). Then the registry phase walks the event directory in byte order of file names until one entry stops the walk, and only after that are its results folded. So stderr carries the built-in `exited` lines, then the `toolu-registry:` lines in walk order, then the registry `exited` lines.

| Situation | Outcome |
|---|---|
| Plugin definitively not installed (Claude `installed_plugins.json`, Codex snapshot) | skipped, inactive. An unreadable record fails open, and Cursor and Hermes count as installed |
| Manifest whose matcher or `applies` does not fit | skipped, not matching |
| `.js` module | skipped, unsupported |
| `deny` before a tool, `{"decision":"block"}` after it | recorded, then the walk stops |

**Before a tool.** The first deny is emitted exactly as its module wrote it. A module exit of 2 blocks with that module's stderr. The first ask is held and receives every advisory. Advisories are deduped and merged into one `additionalContext` and one `systemMessage`. On Codex an `ask` degrades to a deny where the host cannot ask.

**After a tool.** The first block is emitted as written and stops the walk. `permissionDecision` means nothing after the tool ran: no ask is held and no deny stops the walk. The merged object names `hookEventName: "PostToolUse"`.

**Edits.** Edit, Write, MultiEdit and `apply_patch` are walked once per affected path as a synthetic `Edit`. A deny, a block or an exit 2 on any path wins for the whole patch. Unparseable `apply_patch` headers are denied before the tool and blocked after it.

**Payload.** The payload is parsed as `JSON.parse` would read it. Lone surrogate escapes read as U+FFFD, so gates still see the tool, while modules get the raw text. A payload that is not UTF-8, or is nested past 128 levels, fails closed.

**Failure.** An unexpected engine error exits 2: before a tool it blocks the action, and after it the model sees that the post-tool checks did not run.

`toolu hook agent-tier` records delegation telemetry and advises on a model that differs from the running or next ledger step. `toolu hook mcp-tools` runs only mcp-blocker. `fixtures/dispatch/cases.json` holds the walk's shared cases, replayed by `crates/core/engine/tests/dispatch_fixture.rs`; the A, B and C gate goldens are replayed by `crates/cli/tests/golden_gates.rs`.

## Language-quality rules

ts-quality, python-quality and rust-quality (#426–#428) are rule crates on `toolu_engine::quality` (#459). Each implements `QualityRule`: language and extensions, project enablement, gate source and reason, linked-worktree policy, structural YAML, and a per-file check. The engine selects the owners of an edit, scans the live files once with ast-grep, passes each check only its file's matches, and records or clears that rule's gate entry through `toolu-state`. A move clears the source's entry and checks the destination; a permitted linked-worktree edit writes state under that worktree. `fixtures/quality/runner.json` holds the runner's cases.

- **ts-quality** (`crates/ts-quality`) runs in a project with a tracked `tsconfig*.json`, a lock file, and that package manager on `PATH`. Its structural rules are inline YAML; `git grep` and jscpd run with a 10 s deadline.
- **python-quality** (`crates/python-quality`) runs in a project with a top-level `pyproject.toml`, `setup.py`, `setup.cfg` or `requirements.txt` and `python3` on `PATH`, including linked worktrees. Its no-mocks rule scans only `test_*.py` and `*_test.py` files.
- **rust-quality** (`crates/rust-quality`) runs in a project with `Cargo.toml` at the toplevel and `cargo` on `PATH`, and reads `rust-unsafe-exemptions.txt` from the settings directory. `toolu rust-quality check [<path>...]` runs the same rules over the git-listed `.rs` files and is the gate's `rust-quality` step.

**Parity.** `fixtures/quality/{ts,python,rust}-golden.json` hold 120, 97 and 120 captures of the bash modules each rule replaced. `crates/cli/tests/golden_quality.rs` registers each plugin with the built `toolu`, replays every case through `toolu hook post-tools`, and requires the same stdout, stderr, exit code and gate and telemetry files. Three deviations from bash are named and tested: with no `jq` on `PATH` the rule still checks the file (`DEV-1`); an unreadable regular file is a quality violation, not an empty file; and a linked-worktree file is recorded in that worktree's own gate file.

## ast-grep rules

The ast-grep plugin's native SessionStart hook publishes two manifests. `crates/ast-grep` supplies both compiled rules:

- **search-nudge** (`tool/pre`) advises on structural Grep patterns and Bash or Shell file searches. It uses `toolu-shell` to tell a file search from a grep filtering a pipe, and honors `skills.ast-grep: false`.
- **byte-savings** (`tool/post`) appends `{"kind","returned","full"}` records to `<config>/toolu/byte-savings/<session>.jsonl` for Read, Grep, Glob and ast-grep commands. Hosts stay silent about it. `toolu ast-grep savings <ledger.jsonl>` prints the report on demand.

The compiled rules run only when their manifests exist and the plugin is selected. `fixtures/ast-grep/{nudge,savings,golden}.json` and the native hook integration tests cover the behavior. `cargo xtask bench-hooks` measures every hook entry; budgets are in [resource-budgets.md](resource-budgets.md).
