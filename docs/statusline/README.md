# statusline — Gate-Aware Terminal Statusline

**Type:** UI | **Version:** 0.3.1 | **Standalone** (no plugin dependencies)

An optional Claude Code statusline. One line, assembled defensively from the statusline JSON Claude Code sends on stdin — segments degrade gracefully when their data source is absent.

## Install

```text
/plugin install statusline@toolu
```

Then wire it once — easiest with the bundled setup command:

```text
/statusline:setup
```

This adds the `statusLine` key to your `settings.json` idempotently:
- Backs the file up first
- Never clobbers an existing custom `statusLine` (re-run with `--force` to override)
- No-op once wired

Restart the session afterwards for the bar to appear.

### Manual Setup

```json
{
  "statusLine": {
    "type": "command",
    "command": "bash ~/.claude/statusline/statusline.sh"
  }
}
```

Use `$CLAUDE_CONFIG_DIR/statusline/statusline.sh` if you run with a custom config dir. The symlink is refreshed every session, so plugin updates are picked up automatically.

## What It Provides

A defensive one-line status bar:

```
model | effort:high | ctx:45k/200k (22%) | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [CAVEMAN]
```

### Segments

| Segment | Source | Shows When |
|---------|--------|------------|
| `model` | `.model.display_name` | Always |
| `effort` | `.effort.level` | Model reports an effort level |
| `ctx` | `.context_window.*` | Always — current/total usage + % |
| `✗ gate:failing` | `.claude/tmp/quality-gate-status.json` at git root | Quality gate is failing |
| `folder` + `branch` + `↑↓` + `[+~?]` | git, from workspace dir | Inside a git repo — `↑N↓M` shows ahead/behind of the tracked remote, `[+N ~N ?N]` shows staged/unstaged/untracked file counts (both omitted when clean and up-to-date) |
| `[COMEMORY:N]` | `${CLAUDE_CONFIG_DIR}/comemory-status/<repo>.json` | Comemory plugin published a memory count |
| `[CAVEMAN]` | `${CLAUDE_CONFIG_DIR}/.caveman-active` | Caveman plugin is active |

### Degradation

The gate, comemory, git status, and caveman segments degrade gracefully — if the file they read is absent, the segment simply doesn't render. So statusline is **standalone**: it declares no plugin dependencies. Those segments just light up automatically when the relevant plugins are also installed.

## Migrating from toolu ≤ 1.5.0

The statusline used to ship inside the `toolu` plugin and auto-symlinked to `~/.claude/toolu/statusline.sh`. It now lives here. To keep your statusline:

```text
/plugin install statusline@toolu
```

Then re-point `settings.json` from `~/.claude/toolu/statusline.sh` to `~/.claude/statusline/statusline.sh` — `/statusline:setup --force` does this for you (the old path is a custom value to it, so plain `/statusline:setup` would refuse).

## Hooks

| Hook | Event | Purpose |
|------|-------|--------|
| `session-start` | SessionStart | Symlinks `statusline.sh` to stable path |

## Testing

`bats plugins/statusline/__tests__` — real statusline JSON payloads on stdin, no mocks.
