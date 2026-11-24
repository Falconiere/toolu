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
`bash-commands` PreToolUse gate (`toolu_engine::gates`) performs argv-aware
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
| `bash-allowlist.txt`          | `bash-commands` gate (`toolu_engine::gates`)        | Explicit overrides on top of the denylist (deny + allow → allowed). |
| `bash-denylist.txt`           | `bash-commands` gate (`toolu_engine::gates`)        | Tokens the bash guard rejects via argv-aware parsing.              |
| `code-edit-rules.json`        | `code-edit-rules` gate (`toolu_engine::gates`)      | Pattern rules for Write/Edit gating on source files.               |
| `commit-prefixes.txt`         | `commit-gate` gate (`toolu_engine::gates`)          | Allowed Conventional Commits prefixes for `git commit` messages.   |
| `toolu.config.example.json` | (reference — copy to `~/.claude/toolu.config.json`) | Example runtime opt-out config (skills/hooks/mcp). See `docs/config.md`. |
| `protected-files.txt`         | `protected-files` gate (`toolu_engine::gates`)      | Paths the edit guard refuses to modify (lockfiles, secrets, etc.). |
| `rust-unsafe-exemptions.txt`  | `crates/rust-quality/src/rule.rs`                 | Files/paths exempt from the `unsafe` Rust check.                   |

Each plain-text file is one entry per line, `#` for comments. JSON files
follow whatever schema the consuming script documents.

## Runtime config

For per-skill, per-hook, or per-MCP opt-out without touching the data
files above, see `docs/config.md`. The config file lives at
`~/.claude/toolu.config.json` (or the project-local override) and is
deep-merged at runtime. MCP gating was retired with the mcp-blocker gate:
`mcp.<server>: false` still parses but enforces nothing.
