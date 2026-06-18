# statusline

An optional Claude Code statusline. One line, assembled defensively from the
statusline JSON Claude Code sends on stdin:

```
model | effort:high | ctx:45k/200k (22%) | wk:13.7M $1.45 | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [CAVEMAN]
```

| Segment | Source | Shows when |
|---------|--------|------------|
| model | `.model.display_name` | always |
| effort | `.effort.level` | the model reports an effort level |
| ctx | `.context_window.*` | always |
| `wk:` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/statusline/usage/<week>/*.json` | any token usage has been recorded this week |
| `✗ gate:failing` | `.claude/tmp/quality-gate-status.json` at the git root | a **gate writer** (e.g. the `rust-quality` / `ts-quality` / `toolu` plugins) marks the gate failing |
| folder + branch + status | git, from the workspace dir | inside a git repo — `↑N↓M` shows ahead/behind of the tracked remote, `[+N ~N ?N]` shows staged/unstaged/untracked file counts (both omitted when clean and up-to-date) |
| `[COMEMORY:N]` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory-status/<repo>.json` | the **comemory** plugin published a memory count this session |
| `[CAVEMAN]` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/.caveman-active` | the **caveman** plugin is active |

The gate, usage, comemory, git status, and caveman segments degrade gracefully — if the file
they read is absent, the segment simply doesn't render. So statusline is
**standalone**: it declares no plugin dependencies. Those segments just light up
automatically when the relevant plugins are also installed.

### Weekly token usage (`wk:`)

`wk:` is the account-wide tokens consumed **this ISO week (Mon–Sun, local time)** —
`input + output + cache_creation` summed across every session (main agent **and**
subagents), deduped by message id, with `cache_read` excluded (it is ~98% of raw
usage, billed ~0.1×, and does not pace the rate-limit window). A `Stop` hook
(`hooks/token-ledger.sh`) does the transcript parsing once per turn and writes a
small per-`(week, session)` file; the render only sums those files, so it stays
cheap. Each Monday a new week bucket starts and the display resets; prior weeks
are kept on disk under `statusline/usage/` for future reporting, never deleted.

> Week boundaries follow the **local** timezone (the hook buckets each message by
> its own timestamp's local ISO week). Tests pin `TZ=UTC` only for determinism.

## Install & wire up

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
