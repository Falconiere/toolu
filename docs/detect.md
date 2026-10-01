# Detect layer

`@toolu/core/detect` gives the TypeScript gates the answers formerly supplied by `detect.sh`, with parity against the historical Bash version on the same inputs. Those answers are:

- which project markers and linters a repository has;
- whether a tool is on PATH;
- how many lines of code a file holds;
- how a branch is named;
- which repository and branch a `git push` targets.

Issue: [#254](https://github.com/Falconiere/toolu/issues/254). Epic: [#247](https://github.com/Falconiere/toolu/issues/247).

The layer shells out to `git` only. It never runs `wc`, `grep`, `awk` or `jq`. Importing it loads neither the shell parser (unbash) nor zod.

## API

| Export | bash | Answers |
|---|---|---|
| `projectToplevel(o)` | `detect_project_root` | The git toplevel of `o.cwd`, or `undefined` |
| `projectName(o)` | `detect_project_name` | The toplevel's basename |
| `nodePackageManager(o)` | `detect_node_pm` | `bun` (`bun.lock`, `bun.lockb`), `pnpm`, `yarn`, `npm`, in that order |
| `detectRust(o)` / `detectPython(o)` | `detect_rust` / `detect_python` | A `Cargo.toml`, or one of `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt`, at the root |
| `detectTs(o)` | `detect_ts` | git tracks a `tsconfig*.json`. The same pathspecs go to `git ls-files`, so a `*` also matches `/` |
| `tsLinter(o)` | `detect_ts_linter` | `biome`, then `oxc`, then `eslint` (`.eslintrc*`, `eslint.config.*`, file or directory) |
| `pythonLinter(o)` | `detect_python_linter` | `ruff`: a ruff config, or a line starting with `[tool.ruff` in `pyproject.toml` |
| `detectClippy(o)` | `detect_clippy` | A `clippy.toml` or `.clippy.toml` |
| `toRelativePath(path, o)` | `to_relative_path` | `path` relative to the toplevel when it is under it, else unchanged |
| `toolAvailable(name, env)` / `detectAstGrep(env)` | `command -v` / `detect_ast_grep` | A PATH scan, cached per PATH and name |
| `countCodeLines(path)` | `count_code_lines` | Lines left once blanks, `//` and `/* */` are removed. An unclosed `/*` falls back to the raw line count |
| `countPythonCodeLines(path)` | `count_python_code_lines` | Lines that are neither blank nor start with `#` |
| `hasUnterminatedBlock(path)` | `has_unterminated_block` | More `/*` than `*/` |
| `branchSlug(branch)` / `baseBranch(root, env, cwd)` | `branch_slug` / `detect_base_branch` | The state-file key, and origin's HEAD branch else `main` |
| `isGitPush(a)` / `isGitCommit(a)` | `is_git_push` / `is_git_commit` | The command runs `git push` / `git commit` |
| `pushTargetRoot(a, o)` | `push_target_root` | The toplevel of the first push's cumulative `-C` chain. Otherwise the cwd's toplevel, then the host project root, then the cwd |
| `pushTargetBranch(a, root, env)` | `push_target_branch` | The checked-out branch; on a detached HEAD, the refspec's destination, else `""` |

The `o` argument is `{ env?, cwd? }`, defaulting to `process.env` and the process cwd. A counter returns `undefined` when the file cannot be read, where awk printed nothing.

The git questions take a `ShellAnalysis`, not a command string. A gate parses once per event (`shellAnalysisOf(event)` from `@toolu/core/shell`) and hands the analysis to every question. `isGitPush` and `isGitCommit` return `false` when `runsGitSubcommand` answers `unknown` (`$g push`), as bash does for the workflow gates. A gate that must fail closed asks `runsGitSubcommand` for the tristate.

## Ported elsewhere

| bash | TypeScript |
|---|---|
| `detect_plugin_installed`, `toolu_plugin_active` | `pluginPresence`, `pluginActive` in `@toolu/core/registry` |
| `read_list`, `toolu_settings_dir` | `readList`, `settingsDir` in `@toolu/core/config` |
| `strip_heredocs`, `_toolu_split_statements`, `_toolu_statement_tokens`, `_toolu_git_subcommand`, `toolu_runs_git_subcommand` | `analyzeShell`, `runsGitSubcommand` in `@toolu/core/shell` |
| `bash_write_targets` | `writeTargets` in `@toolu/core/shell/writes` |

Function-size counting is not in `detect.sh`. Each language plugin's `50-size-fn.sh` ports with its plugin (#265, #266, #267).

## Parity

The native functions are tested in real git repositories. The former `detect.sh` is available at tag `v7.2.0` for historical comparison:

- **Project probes** (`detect-project.test.ts`): 35 repository layouts, from the root and from a subdirectory, plus outside a repository.
- **Line counters** (`detect-lines.test.ts`): every tracked `*.ts`, `*.rs`, `*.py` and `*.sh` file in this repository, plus edge snippets and a file over 1 MiB.
- **Git questions**: `detect-git.test.ts` and `detect-branch.test.ts` cover push, commit, branch, and worktree behavior. The shell parser cases live in `shell/__tests__/shell-git.test.ts` and `shell-rules.test.ts`.

The former Bats coverage has been replaced by colocated Bun tests for each native function.

Allowed differences:

- **#283 items 8 and 9**, pinned by fixtures `283-8a`…`283-8n`, `283-9a` and `283-9b`. Pushes and commits hidden behind wrappers, `bash -c`, `eval`, a line continuation or a heredoc substitution are seen. A `-C` path with spaces, and a `-C` on another command in the chain, resolve the right repository.
- **NUL bytes.** macOS awk ends a record at NUL, and `grep -o` prints one "Binary file matches" line for a file holding one, so bash answers differently per platform. The port counts what is written: `"/* /* */\0\nd\n"` is `2` code lines, `2` Python lines, and an unterminated block.

`command -v` quirk kept: when PATH holds a non-executable file of that name and no executable one, bash still reports it, so `toolAvailable` does too. A name holding `/` must be executable.

## Cost

Reads are chunked (64 KiB), so memory stays flat. Counting a 256 MiB file grows RSS by the same ~30 MiB of heap slack as a 16 MiB one (`detect-lines.test.ts`).

`detect-import-cost.test.ts` bundles the entry with the plugin pipeline's flags. It then times `import()` in fresh `bun` processes that have already loaded `node:fs`, `node:child_process` and `node:path`, as every hook has. Budget: p50 under 2 ms. Recorded on 2026-09-29 (macOS arm64, Bun 1.4.2, a machine shared with parallel workers):

| Measure | p50 |
|---|---|
| Bundle size | 17.9 KB |
| detect import, builtins loaded | 0.59–1.27 ms |
| Empty module, builtins loaded | 0.25–0.72 ms |
| detect import, cold (builtins loaded by it) | 2.9–3.9 ms |
