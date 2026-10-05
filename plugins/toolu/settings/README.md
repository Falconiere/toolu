# settings/

Reusable Claude Code settings fragments plus data files consumed by hooks.

## Fragments

- `permissions.fragment.json` — sanitized union of permission allowlists/denylists from the source repos. Its `deny` block is deliberately short: `permissions.deny` **overrides** a PreToolUse hook's `ask`, so a rule here silently outranks whatever `gates.<name>.mode` says. Only catastrophic, never-ask-me commands belong in it (`rm -rf /`, `mkfs`, `dd`, `sudo rm`, `chmod 777`); `git push --force` and `node -e` moved to `bash-denylist.txt`, where the gate mode decides whether they deny, ask, or pass. Most repos do not need to merge this at all — toolu writes `Bash(*)`, `Edit`, `Write` into `.claude/settings.local.json` on first run (see `permissions.autoAllow` in `docs/config.md`). This is a **dev-mode merge for the repo checkout**; the `ast-grep` wrapper allow-rules pin the full `plugins/ast-grep/` paths (repo-root and `.claude/worktrees/*` variants) so an untrusted temp/checkout directory with a same-named tail cannot satisfy the allowlist. Installed plugins are governed by Claude Code's own plugin permissions, not this fragment.

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
`bash-commands` PreToolUse gate (`@toolu/core/gates`) performs argv-aware
enforcement using `bash-denylist.txt`. Both layers must be installed for
the security model to hold:

- Settings denies catch obvious cases at the matcher layer.
- `bash-commands` parses the command (no `python3` needed) and applies each
  rule to every simple command on the line: after `&&`, `;` and pipes, in
  subshells, behind env prefixes and wrappers such as `sudo`, and inside
  `bash -c` / `eval`. A line it cannot parse (over the 1 MiB parser cap) is
  treated as a hit, never as allowed.

## Data files (read by hooks)

Consumer paths below are relative to the plugin root (`plugins/toolu/`);
the merge commands above are written for the repo root because cwd matters
when you run them.

| File                          | Consumer                                          | Purpose                                                            |
|-------------------------------|---------------------------------------------------|--------------------------------------------------------------------|
| `bash-allowlist.txt`          | `bash-commands` gate (`@toolu/core/gates`)        | Explicit overrides on top of the denylist (deny + allow → allowed). |
| `bash-denylist.txt`           | `bash-commands` gate (`@toolu/core/gates`)        | Tokens the bash guard rejects via argv-aware parsing.              |
| `code-edit-rules.json`        | `code-edit-rules` gate (`@toolu/core/gates`)      | Pattern rules for Write/Edit gating on source files.               |
| `commit-prefixes.txt`         | `commit-gate` gate (`@toolu/core/gates`)          | Allowed Conventional Commits prefixes for `git commit` messages.   |
| `mcp-blocklist.txt`           | `mcp-blocker` gate (`@toolu/core/gates`)          | MCP server prefixes blocked unconditionally (plain text).          |
| `toolu.config.example.json` | (reference — copy to `~/.claude/toolu.config.json`) | Example runtime opt-out config (skills/hooks/mcp). See `docs/config.md`. |
| `protected-files.txt`         | `protected-files` gate (`@toolu/core/gates`)      | Paths the edit guard refuses to modify (lockfiles, secrets, etc.). |
| `rust-unsafe-exemptions.txt`  | `rust-quality/hooks/src/post-tool-use.ts`         | Files/paths exempt from the `unsafe` Rust check.                   |

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
# steer Jira work to the host's native Jira tools when available):
# claude_ai_Atlassian -> use the host's native Jira tools instead
```

The example is shown **commented**, so nothing is blocked out of the box.
Uncomment the line (in your own settings dir) only if you want a hard block too.

Allow/deny semantics for the bash guard: rules apply to each simple command
on the line. A command that matches a deny rule is still allowed if that same
command also matches an allowlist rule (the allowlist is an explicit override,
not a default gate); an allow rule matching a *different* command on the line
does not override it, so `ls && node -e …` with `ls` allowed is still denied.
Commands that match no deny rule are allowed by default.

> Allowlist caveat: single-token allowlist entries match anywhere in a
> command's text as **substrings** (only multi-token entries are argv-aware). A broad single-token entry such as `node` therefore
> overrides far more than intended and quietly broadens the attack surface
> (e.g. it would exempt `node -e '…'`). Prefer specific multi-token entries for
> exemptions so the override stays argv-scoped to exactly the command you mean.

Lookup order (see `toolu_settings_dir` in `hooks/lib/detect.sh`, sibling of
this directory inside the plugin):
`$TOOLU_SETTINGS_DIR` (if set) → `~/.claude/settings` (if it exists) →
the plugin's own `settings/` directory, resolved relative to the hooks. There is
no per-project `.claude/settings/` lookup — to override per project, point
`TOOLU_SETTINGS_DIR` at a project-local directory.

## Env vars

| Variable                  | Effect                                  |
|---------------------------|-----------------------------------------|
| `TOOLU_SETTINGS_DIR`  | Directory the hooks read data files from |
| `MY_CLAUDE_QUALITY`       | `off` to disable the `quality-gate` gate |
| `MY_CLAUDE_COMEMORY_REPO` | Overrides the comemory `--repo` scope used by the comemory wrapper (the comemory plugin's `skills/agent-memory/scripts/comemory.sh`). Defaults to the git project name. Not read by any hook. |

## Statusline

The statusline moved to its own optional plugin, **`statusline`** — it shows
`model | effort | ctx | <gate> | folder | branch`, with a loud red
`✗ gate:failing` marker driven by the same `.claude/tmp/quality-gate-status.json`
the rust-quality / ts-quality / python-quality hooks write. Install it and wire `settings.json` per
`plugins/statusline/README.md`:

```
/plugin install statusline@toolu
```

```json
{ "statusLine": { "type": "command",
                  "command": "~/.claude/statusline/statusline.sh" } }
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
