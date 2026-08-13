# statusline

Host-native project status. Claude Code gets an optional persistent one-line
statusline assembled defensively from the JSON Claude sends on stdin:

```
model | effort:high | ctx:45k/200k (22%) | example.com | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [CAVEMAN]
```

| Segment | Source | Shows when |
|---------|--------|------------|
| model | `.model.display_name` | always |
| effort | `.effort.level` | the model reports an effort level |
| ctx | `.context_window.*` | always |
| `example.com` | `.oauthAccount.emailAddress` in `~/.claude.json` (or `$CLAUDE_CONFIG_DIR/.claude.json`) | logged in via Claude OAuth — shows only the email domain, not the full address |
| `✗ gate:failing` | host-native `.claude/tmp/quality-gate-status.json` at the git root | a **gate writer** (e.g. the `rust-quality` / `ts-quality` / `toolu` plugins) marks the gate failing |
| folder + branch + status | git, from the workspace dir | inside a git repo — `↑N↓M` shows ahead/behind of the tracked remote, `[+N ~N ?N]` shows staged/unstaged/untracked file counts (both omitted when clean and up-to-date) |
| `[COMEMORY:N]` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory-status/<repo>.json` | the **comemory** plugin published a memory count this session |
| `[CAVEMAN]` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.caveman-active` | the **caveman** plugin is active |

Codex exposes `$statusline:status` instead of a persistent bar. It reports the
repository, branch/ahead/behind state, working-tree counts, quality gate from
`<repo>/.codex/tmp/quality-gate-status.json`, and comemory count. It deliberately
omits account, model, effort, and context-window fields that Codex does not make
available to the skill.

The account, gate, comemory, git status, and caveman segments degrade gracefully — if the file
they read is absent, the segment simply doesn't render. So statusline is
**standalone**: it declares no plugin dependencies. Those segments just light up
automatically when the relevant plugins are also installed (or, for the account
segment, when you're logged in via Claude OAuth rather than an API key).

## Codex install

```bash
codex plugin add statusline@toolu
```

Run `$statusline:status` whenever you want a current report. No setup or
persistent renderer is required.

## Claude Code install & wire up

Claude Code does not let a plugin declare `statusLine` in its manifest, so the
SessionStart hook symlinks the script to a stable, version-independent path:

```
~/.claude/statusline/statusline.sh   (→ the installed plugin's statusline.sh)
```

1. Install the plugin:

   ```
   /plugin install statusline@toolu
   ```

2. Wire it once. Easiest — run the bundled command:

   ```
   /statusline:setup
   ```

   It adds the `statusLine` key below to your `settings.json` idempotently:
   it backs the file up first, never clobbers an existing custom statusLine
   (re-run `/statusline:setup --force` if you do want to replace one), and is a
   no-op once wired. Restart the session afterwards for the bar to appear.

   Or wire it by hand:

   ```json
   {
     "statusLine": {
       "type": "command",
       "command": "bash ~/.claude/statusline/statusline.sh"
     }
   }
   ```

   (Use `$CLAUDE_CONFIG_DIR/statusline/statusline.sh` if you run with a custom
   config dir.) The symlink is refreshed every session, so plugin updates are
   picked up automatically with no settings change. The hook never clobbers a
   real file you place at that path — it only owns its own symlink.

## Migrating from toolu ≤ 1.5.0

The statusline used to ship inside the `toolu` plugin and auto-symlinked to
`~/.claude/toolu/statusline.sh`. It now lives here. To keep your statusline:

- `/plugin install statusline@toolu`, and
- re-point `settings.json` from `~/.claude/toolu/statusline.sh` to
  `~/.claude/statusline/statusline.sh` — `/statusline:setup --force` does this
  for you (the old path is a custom value to it, so plain `/statusline:setup`
  would refuse).

Toolu no longer creates the old symlink and sweeps away the dangling one it
used to own, so an un-migrated `settings.json` will fail loudly (missing file)
rather than silently pointing into a cleaned plugin cache.
