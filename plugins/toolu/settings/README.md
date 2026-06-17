# settings/

Reusable Claude Code settings fragments plus data files consumed by hooks.

## Fragments

- `permissions.fragment.json` — sanitized union of permission allowlists/denylists from the source repos. This is a **dev-mode merge for the repo checkout**; the `ast-grep` and `comemory` wrapper allow-rules pin the full `plugins/ast-grep/` and `plugins/comemory/` paths (repo-root and `.claude/worktrees/*` variants) so an untrusted temp/checkout directory with a same-named tail cannot satisfy the allowlist. Installed plugins are governed by Claude Code's own plugin permissions, not this fragment.

Hook wiring ships in the plugin manifest (`plugins/toolu/hooks/hooks.json`);
installing the plugin registers all hooks — no manual settings merge needed.

### Merge into your settings

For the permissions fragment, run from the repo root:

```bash
jq -s '.[0] * .[1]' ~/.claude/settings.json plugins/toolu/settings/permissions.fragment.json \
  > ~/.claude/settings.json.new && mv ~/.claude/settings.json.new ~/.claude/settings.json
```

> Never add a path that contains secrets (e.g. `.env`) to `additionalDirectories`.

## Security note

The settings.json deny matcher is substring-based and unreliable for argv
shapes like `node -e`, `git push --force`, or `git push origin main`. The
`hooks/pre-tools/modules/bash-commands.sh` hook performs argv-aware
enforcement using `bash-denylist.txt`. Both layers must be installed for
the security model to hold:

- Settings denies catch obvious cases at the matcher layer.
- `bash-commands.sh` parses argv and reliably rejects the listed tokens
  even when they are embedded mid-command-line.

## Data files (read by hooks)

Consumer paths below are relative to the plugin root (`plugins/toolu/`);
the merge commands above are written for the repo root because cwd matters
when you run them.

| File                          | Consumer                                          | Purpose                                                            |
|-------------------------------|---------------------------------------------------|--------------------------------------------------------------------|
| `bash-allowlist.txt`          | `hooks/pre-tools/modules/bash-commands.sh`        | Explicit overrides on top of the denylist (deny + allow → allowed). |
| `bash-denylist.txt`           | `hooks/pre-tools/modules/bash-commands.sh`        | Tokens the bash guard rejects via argv-aware parsing.              |
| `code-edit-rules.json`        | `hooks/pre-tools/modules/code-edit-rules.sh`      | Pattern rules for Write/Edit gating on source files.               |
| `commit-prefixes.txt`         | `hooks/pre-tools/modules/commit-gate.sh`          | Allowed Conventional Commits prefixes for `git commit` messages.   |
| `mcp-blocklist.txt`           | `hooks/pre-tools/modules/mcp-blocker.sh`          | MCP server prefixes blocked unconditionally (plain text).          |
| `toolu.config.example.json` | (reference — copy to `~/.claude/toolu.config.json`) | Example runtime opt-out config (skills/hooks/mcp). See `docs/config.md`. |
| `protected-files.txt`         | `hooks/pre-tools/modules/protected-files.sh`      | Paths the edit guard refuses to modify (lockfiles, secrets, etc.). |
| `rust-unsafe-exemptions.txt`  | `hooks/post-tools/modules/rust-quality.sh`        | Files/paths exempt from the `unsafe` Rust check.                   |

Each plain-text file is one entry per line, `#` for comments. JSON files
follow whatever schema the consuming script documents.

### `mcp-blocklist.txt` redirect hints

A blocklist line may carry an optional `-> <text>` suffix after the server
prefix. When that server is denied, the text is appended to the deny reason so
the user is pointed at the replacement. The prefix alone drives the match; the
hint is ignored for matching.

```text
# <server-prefix> -> <redirect text shown in the deny reason>
# Example (commented = inert; uncomment to actually block the Atlassian MCP and
# steer Jira work to the bundled jira skill):
# claude_ai_Atlassian -> use the `jira` skill instead
```

The example is shown **commented**, so nothing is blocked out of the box — the
`jira`/toolu prompt nudge handles discoverability without denying the MCP.
Uncomment the line (in your own settings dir) only if you want a hard block too.

Allow/deny semantics for the bash guard: the denylist is checked first; a
command that matches a deny rule is still allowed if it also matches an
allowlist rule (the allowlist is an explicit override, not a default gate).
Commands that match no deny rule are allowed by default.

> Allowlist caveat: single-token allowlist entries match anywhere in the
> command string as **substrings** (only multi-token entries are argv-aware —
> see `bash-commands.sh`). A broad single-token entry such as `node` therefore
> overrides far more than intended and quietly broadens the attack surface
> (e.g. it would exempt `node -e '…'`). Prefer specific multi-token entries for
> exemptions so the override stays argv-scoped to exactly the command you mean.

Lookup order (see `detect_settings_dir` in `hooks/lib/detect.sh`, sibling of
this directory inside the plugin):
`$MY_CLAUDE_SETTINGS_DIR` (if set) → `~/.claude/settings` (if it exists) →
the plugin's own `settings/` directory, resolved relative to the hooks. There is
no per-project `.claude/settings/` lookup — to override per project, point
`MY_CLAUDE_SETTINGS_DIR` at a project-local directory.

## Env vars

| Variable                  | Effect                                  |
|---------------------------|-----------------------------------------|
| `MY_CLAUDE_SETTINGS_DIR`  | Directory the hooks read data files from |
| `MY_CLAUDE_QUALITY`       | `off` to disable `quality-gate.sh`       |
| `MY_CLAUDE_COMEMORY_REPO` | Overrides the comemory `--repo` scope used by the comemory wrapper (the comemory plugin's `skills/agent-memory/scripts/comemory.sh`). Defaults to the git project name. Not read by any hook. |

## Statusline

The statusline moved to its own optional plugin, **`statusline`** — it shows
`model | effort | ctx | <gate> | folder | branch | <caveman>`, with a loud red
`✗ gate:failing` marker driven by the same `.claude/tmp/quality-gate-status.json`
the rust-quality / ts-quality hooks write. Install it and wire `settings.json` per
`plugins/statusline/README.md`:

```
/plugin install statusline@toolu
```

```json
{ "statusLine": { "type": "command",
                  "command": "bash ~/.claude/statusline/statusline.sh" } }
```

Toolu no longer ships or symlinks the statusline; on upgrade it sweeps the
old `~/.claude/toolu/statusline.sh` symlink it used to own. If you previously
wired that path, re-point it to `~/.claude/statusline/statusline.sh`.

## Runtime config

For per-skill, per-hook, or per-MCP opt-out without touching the data
files above, see `docs/config.md`. The config file lives at
`~/.claude/toolu.config.json` (or the project-local override) and is
deep-merged at runtime. The `mcp-blocklist.txt` blocklist still works in
parallel; either source can block a server.
