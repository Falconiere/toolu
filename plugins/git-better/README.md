# git-better

Semantic branches and commits agents can read. A commit has a scoped Conventional Commit subject and an `Intent:` trailer, and a branch is named `<type>/<ref>-<slug>`. Both are checked before git runs.

## Install

```text
/plugin install git-better@toolu
```

**Prerequisites:** the native [`toolu` binary](../../docs/install.md) and the `toolu` plugin. See [runtime](../../docs/runtime.md).

**Hooks:** the SessionStart hook runs `toolu git-better hook register` from the generated launcher. That publishes the `semantic-git` and `commit-note` manifests in the toolu registry. From then on, toolu's own PreToolUse and PostToolUse dispatchers run the compiled-in rules on every Bash/Shell command. A second SessionStart entry, `check-toolu`, warns on Codex when the `toolu` plugin is missing.

Skills write plain `toolu git-better <verb>`, never a fixed binary path. See [docs/install.md](../../docs/install.md#agent-command-shell).

Requires the `toolu` plugin.

Keep `name`, `version`, and `description` identical in `.claude-plugin/plugin.json`, `.codex-plugin/plugin.json` and `.muse-plugin/plugin.json`.

## Crate

`crates/git-better` is this plugin's Rust library.
- **Namespace:** it provides `toolu git-better`.
  - `check` validates a commit message (`-m`, `--message-file FILE|-`) and a branch name (`--branch`), and exits 1 when one fails.
  - `setup` adds `refs/notes/git-better` to the repository's `notes.rewriteRef`, so amend and rebase carry the notes. See the [CLI reference](../../docs/cli/README.md).
- **Rules:** the `semantic-git` and `commit-note` registry rules are linked into the hub, `crates/toolu`.

## What it provides

- **`git-better:git-better`** is the skill. It covers the commit and branch format, how to pass a message the rule can read, and how to fix a blocked commit.
- **The `semantic-git` rule** reads each `git commit`, `checkout -b`, `switch -c`, `branch` and `worktree add -b` from the parsed command line.
  - **Commit subject:** `type(scope): description`, at most 72 characters.
  - **Trailers:** `Intent:` is required. `Risk: low|medium|high` and `Refs:` are optional. A breaking change needs both `!` and `BREAKING-CHANGE:`.
  - **Branch name:** `<type>/[<ref>-]<slug>`.
  - **Message sources:** `-m` values, `-F` files, heredocs on `-F -`, and `--trailer` values.
  - **Always allowed:** `fixup!`/`squash!` commits and reused messages. `wip:` is allowed only off the base branch.
  - **Exempt branches:** existing branches, the base branch and `release-please--*`.
  - **Not readable before the command runs** (`-m "$MSG"`, a pipe, the editor): the rule explains how to check first and does not block.
- **The `commit-note` rule** runs after a successful `git commit`. It attaches a JSON note under `refs/notes/git-better`.
  - **Contents:** the branch, base, host and session the commit was made on, plus its changed areas and files.
  - **Why:** a squash merge or rebase loses that provenance, while trailers stay the source of truth.
  - **Sharing:** notes stay local unless you push `refs/notes/git-better` yourself.
- **Configuration:** `gitBetter` in `toolu.config.json` sets `mode` (`block` by default, `advise` or `off`), `requireScope`, `requireRef`, `allowWip`, `types`, `notes` and `areaRoots`. See [docs/config.md](../../docs/config.md#semantic-branches-and-commits-with-git-better).
  - A value the plugin cannot read keeps its default and is reported.
  - On hosts that cannot tell whether a plugin is installed (Muse, Cursor), the manifest outlives an uninstall. Set `gitBetter.mode` to `off` there.
  - Upgrade the `toolu` binary before adding a `gitBetter` section: an older binary rejects the unknown key.

## Muse

The Muse manifest uses generated hooks and resolvers. Validate with `muse plugins validate plugins/git-better`.
