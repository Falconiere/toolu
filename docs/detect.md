# Detect layer

`toolu_state::detect` and `toolu_engine::detect` give the native gates the answers formerly supplied by `detect.sh`, with parity against the historical Bash version on the same inputs. Those answers are:

- which project markers and linters a repository has;
- whether a tool is on PATH;
- how many lines of code a file holds;
- how a branch is named;
- which repository and branch a `git push` targets.

Issue: [#254](https://github.com/Falconiere/toolu/issues/254). Epic: [#247](https://github.com/Falconiere/toolu/issues/247).

The layer shells out to `git` only, and for most questions not even that (see **Git facts without git**). It never runs `wc`, `grep`, `awk` or `jq`.

## Host detection

`toolu_runtime::host::detect` picks the host that spawned a hook; the first hit wins:

1. `TOOLU_HOST_OVERRIDE` (an invalid value warns and falls through);
2. a hook event name only one host uses;
3. Cursor's `CURSOR_VERSION` or `CURSOR_PROJECT_DIR`;
4. Muse's `MUSE_PLUGIN_ID` or `MUSE_SESSION_ID`;
5. Codex's `PLUGIN_ROOT`;
6. otherwise Claude Code.

On Muse the config root is `MUSE_CONFIG_DIR`, else `$XDG_CONFIG_HOME/muse`, else `~/.config/muse`; `TOOLU_CONFIG_DIR` wins on every host. The plugin root and data directory are `MUSE_PLUGIN_ROOT` and `MUSE_PLUGIN_DATA_DIR`. Muse has no project-directory variable, so the project root is the git toplevel.

## API

| Function | bash | Answers |
|---|---|---|
| `project_toplevel(env, cwd)` | `detect_project_root` | The git toplevel of `cwd`, or `None` |
| `project_name(env, cwd)` | `detect_project_name` | The toplevel's basename |
| `node_package_manager(env, cwd)` | `detect_node_pm` | `bun` (`bun.lock`, `bun.lockb`), `pnpm`, `yarn`, `npm`, in that order. It names the user project's package manager |
| `detect_rust` / `detect_python` | same | A `Cargo.toml`, or one of `pyproject.toml`, `setup.py`, `setup.cfg`, `requirements.txt`, at the root |
| `detect_ts` | `detect_ts` | git tracks a `tsconfig*.json`. The same pathspecs go to `git ls-files`, so a `*` also matches `/` |
| `ts_linter` | `detect_ts_linter` | `biome`, then `oxc`, then `eslint` (`.eslintrc*`, `eslint.config.*`, file or directory) |
| `python_linter` | `detect_python_linter` | `ruff`: a ruff config, or a line starting with `[tool.ruff` in `pyproject.toml` |
| `detect_clippy` | `detect_clippy` | A `clippy.toml` or `.clippy.toml` |
| `to_relative_path(path, env, cwd)` | `to_relative_path` | `path` relative to the toplevel when it is under it, else unchanged |
| `tool_available(name, env)` / `detect_ast_grep(env)` | `command -v` / `detect_ast_grep` | A PATH scan, cached per PATH and name |
| `count_code_lines(path)` | `count_code_lines` | Lines left once blanks, `//` and `/* */` are removed. An unclosed `/*` falls back to the raw line count |
| `count_python_code_lines(path)` | `count_python_code_lines` | Lines that are neither blank nor start with `#` |
| `has_unterminated_block(path)` | `has_unterminated_block` | More `/*` than `*/` |
| `branch_slug(branch)` / `base_branch(env, root, cwd)` | `branch_slug` / `detect_base_branch` | The state-file key, and origin's HEAD branch else `main` |
| `is_git_push(a)` / `is_git_commit(a)` (`toolu_engine::detect`) | `is_git_push` / `is_git_commit` | The command runs `git push` / `git commit` |
| `push_target_root(a, roots, cwd)` | `push_target_root` | The toplevel of the first push's cumulative `-C` chain. Otherwise the cwd's toplevel, then the host project root, then the cwd |
| `push_target_branch(a, root, env)` | `push_target_branch` | The checked-out branch; on a detached HEAD, the refspec's destination, else `""` |

The environment is an explicit `Env` snapshot. A counter returns `None` when the file cannot be read, where awk printed nothing.

The git questions take a `ShellAnalysis`, not a command string. The engine parses once per event and hands the analysis to every question. `is_git_push` and `is_git_commit` return `false` when `runs_git_subcommand` answers `unknown` (`$g push`), as bash does for the workflow gates. A gate that must fail closed asks `runs_git_subcommand` for the tristate.

## Where the answers live

`toolu_state::detect` (#415, `crates/core/state/src/detect/`) holds the shell-free answers above: `project` (toplevel, name, package manager, markers, linters, `to_relative_path`), `tools` (`tool_available`, `detect_ast_grep`) and `lines` (the three counters).

The push and commit questions need the shell parser, so they live one layer up in `toolu_engine::detect` (#418, `crates/core/engine/src/detect.rs`): `is_git_push`, `is_git_commit`, `push_target_root` and `push_target_branch`, on `toolu_shell::git`. The `-C` chain is replayed from the cwd, each step relative to the last, and its toplevel is read from `.git`, so no git process is spawned unless the walk defers to git. `crates/core/engine/tests/detect.rs` checks them on real repositories, including a linked worktree in a directory with a space and detached and unborn HEADs.

**Git facts without git.** The toplevel comes from `toolu_runtime::git`, which walks up from the physical cwd for `.git` as git's own discovery does. It handles:
- linked worktrees, submodules and `--separate-git-dir` pointers;
- bare repositories and the inside of `.git`;
- filesystem boundaries;
- `core.bare` and `core.worktree`.

`toolu_state::git` reads the branch (`HEAD`, loose refs and `packed-refs`), the linked-worktree test, the common dir and origin's HEAD the same way. So `Roots::project_root` and these facts spawn no process. As in git, a linked worktree ignores the shared config's `core.bare` and `core.worktree`.

These cases ask git, as the removed TypeScript layer always did:
- **Environment:** a `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_OBJECT_DIRECTORY`, `GIT_CEILING_DIRECTORIES`, `GIT_DISCOVERY_ACROSS_FILESYSTEM` or `GIT_CONFIG*` variable, even an empty one, and `sudo` as root.
- **Ownership:** a worktree, `.git` file or git dir owned by another user (`safe.directory`).
- **Config:**
  - an include or a malformed section header;
  - a quoted or escaped `core.bare`, `core.worktree` or `core.repositoryformatversion`;
  - an empty or valueless `core.worktree`, or one that does not resolve;
  - a repository format above 1;
  - a config that is not UTF-8 or cannot be read;
  - a `config.worktree` file.
- **Refs:**
  - reftable repositories;
  - a `HEAD` that is a symlink, cannot be read, or names a ref outside `refs/heads/`;
  - a symlinked `refs/remotes/origin/HEAD`;
  - a branch whose short name git would lengthen: one shared by a file at the git dir's root, `refs/<name>`, a tag, a remote, or `refs/remotes/<name>/HEAD`.

Git is still spawned for history and index questions, which run once per session:
- `diff_sha` (`git diff`, `git hash-object`);
- the sweeper's branch lists (`git branch [--merged]`);
- `detect_ts` (`git ls-files`).

## Ported elsewhere

| bash | Rust |
|---|---|
| `detect_plugin_installed`, `toolu_plugin_active` | plugin presence in `toolu_engine::registry` |
| `read_list`, `toolu_settings_dir` | `toolu_runtime::config` |
| `strip_heredocs`, `_toolu_split_statements`, `_toolu_statement_tokens`, `_toolu_git_subcommand`, `toolu_runs_git_subcommand` | `analyze`, `runs_git_subcommand` in `toolu_shell` |
| `bash_write_targets` | `write_targets` in `toolu_shell::writes` |

Function-size counting is not in `detect.sh`. Each language plugin's `50-size-fn.sh` ported with its plugin (#265, #266, #267).

## Parity

The native functions are tested in real git repositories. The former `detect.sh` is available at tag `v7.2.0` for historical comparison:

- **Project probes** (`crates/core/state/tests/detect.rs`): 35 repository layouts, from the root and from a subdirectory, plus outside a repository.
- **Line counters** (`crates/core/state/src/detect/tests/lines_test.rs`): edge snippets and a file over 1 MiB. The run that compared Rust and TypeScript counts on every tracked file ended with the TypeScript tree (#440).
- **Git questions**: `crates/core/engine/tests/detect.rs` and `crates/core/state/tests/git_facts.rs` cover push, commit, branch, and worktree behavior. The shell parser cases live in `crates/core/shell/tests/`.

Allowed differences:

- **#283 items 8 and 9**, pinned by fixtures `283-8a`…`283-8n`, `283-9a` and `283-9b`. Pushes and commits hidden behind wrappers, `bash -c`, `eval`, a line continuation or a heredoc substitution are seen. A `-C` path with spaces, and a `-C` on another command in the chain, resolve the right repository.
- **NUL bytes.** macOS awk ends a record at NUL, and `grep -o` prints one "Binary file matches" line for a file holding one, so bash answers differently per platform. The Rust counters count what is written: `"/* /* */\0\nd\n"` is `2` code lines, `2` Python lines, and an unterminated block.

`command -v` quirk kept: when PATH holds a non-executable file of that name and no executable one, bash still reports it, so `tool_available` does too. A name holding `/` must be executable.
