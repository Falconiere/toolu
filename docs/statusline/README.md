# statusline — Gate-Aware Terminal Statusline

**Type:** Status | **Version:** 6.5.0 | **Standalone** (no plugin dependencies)

Host-native status. Claude Code gets an optional persistent statusline; Codex
gets an explicit `$statusline:status` skill, and OpenCode the native
`statusline-status` skill. All consume the same repository, git, quality-gate,
comemory, and Jev readiness collector; OpenCode's report adds toolu's own
plugin readiness.

## Install

```text
/plugin install statusline@toolu
```

For Codex:

```bash
codex plugin add statusline@toolu
```

Run `$statusline:status`. Codex reports only locally available repository,
branch, working-tree, gate (`<repo>/.codex/tmp/quality-gate-status.json`), and
comemory and Jev readiness state; it does not fabricate Claude-only model, effort, account, or
context-window values. The remaining setup below applies only to Claude's
persistent renderer.

Then wire it once — easiest with the bundled setup command:

```text
/statusline:setup
```

This adds the `statusLine` key to your `settings.json` idempotently:
- Backs the file up first
- Never clobbers an existing custom `statusLine` (re-run with `--force` to override)
- Updates the older `bash ~/.claude/statusline/statusline.sh` value, which cannot run the Bun renderer
- No-op once wired

Restart the session afterwards for the bar to appear.

### Manual Setup

```json
{
  "statusLine": {
    "type": "command",
    "command": "~/.claude/statusline/statusline.sh"
  }
}
```

Use `"$CLAUDE_CONFIG_DIR/statusline/statusline.sh"` if you run with a custom config dir. The path is a symlink to the plugin's Bun renderer (`hooks/dist/statusline.js`), refreshed every session, so plugin updates are picked up automatically. Run it by path, never through `bash`; while `settings.json` still holds the old `bash` form, SessionStart prints a notice to run `/statusline:setup`.

## OpenCode

On OpenCode the native `statusline-status` skill is the status surface. OpenCode
has no `statusLine` setting, so the persistent statusline and
`/statusline:setup` are Claude Code-only: toolu does not generate the setup
command for OpenCode and never writes a `statusLine` key into OpenCode
configuration. The skill runs
`"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"`.
`shell.env` sets both variables in every bash call. The report starts with toolu's
own readiness:

```
Host: OpenCode
toolu: ready — 2 plugins (project selection), 2 startup artifacts
Plugins: statusline (session-start), jev (session-start, user-prompt-submit)
Startup record: <project>/.opencode/toolu/state/toolu/opencode-status.json, written <time> for <project>
Repository: …
```

Every startup of the toolu OpenCode plugin writes that record
(`<data root>/toolu/opencode-status.json`). It also sends one structured
`toolu: status` entry to the host log (`opencode --print-logs`) with `status`,
`plugins`, `selection`, `artifacts`, `record` and, when toolu is not ready,
`reason`. Neither holds environment values. A missing or unreadable record prints
a line naming the next step. When toolu is not ready, it denies every tool call,
bash included. Its reason then reaches you through the denial and the host log,
and the record keeps it for a later `status.js` run with `TOOLU_CONFIG_DIR` set to
the data root.

## What It Provides

A defensive one-line status bar:

```
model | effort:high | ctx:45k/200k (22%) | example.com | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [JEV:READY]
```

### Segments

| Segment | Source | Shows When |
|---------|--------|------------|
| `model` | `.model.display_name` | Always |
| `effort` | `.effort.level` | Model reports an effort level |
| `ctx` | `.context_window.*` | Always — current/total usage + % |
| `example.com` | `.oauthAccount.emailAddress` in `~/.claude.json` (or `$CLAUDE_CONFIG_DIR/.claude.json`) | Logged in via Claude OAuth — shows only the email domain, not the full address |
| `✗ gate:failing` | `.claude/tmp/quality-gate-status.json` at git root | Quality gate is failing |
| `folder` + `branch` + `↑↓` + `[+~?]` | git, from workspace dir | Inside a git repo — `↑N↓M` shows ahead/behind of the tracked remote, `[+N ~N ?N]` shows staged/unstaged/untracked file counts (both omitted when clean and up-to-date) |
| `[COMEMORY:N]` | `${CLAUDE_CONFIG_DIR}/comemory-status/<repo>.json` | Comemory plugin published a memory count |
| `[JEV:READY]` / `[JEV:UNAVAILABLE: reason]` | Published Jev wrapper, Bun, and environment API key | Wrapper is published; green when locally ready, yellow with the reason when unavailable |

Jev readiness checks `<config-dir>/jev/jev.sh`, Bun, and a nonempty
`TYPESAFE_API_KEY` without line breaks. Config resolution honors
`TOOLU_CONFIG_DIR`, then the active host's `CLAUDE_CONFIG_DIR` or `CODEX_HOME`,
then its default directory. It never prints the key, reads `.env`, runs the
wrapper, or calls the API. **Ready means locally configured**, not verified
authentication or service health. Missing prerequisites show a reason such as
`missing TYPESAFE_API_KEY`, `invalid TYPESAFE_API_KEY`, `missing bun`, or
`missing executable wrapper`; multiple reasons are separated by semicolons.
No published wrapper means no Jev segment. Codex reports the same information
as `Jev: ready` or `Jev: unavailable — reason`.

### Degradation

The account, gate, comemory, and git status segments degrade gracefully — if the file they read is absent, the segment simply doesn't render. So statusline is **standalone**: it declares no plugin dependencies. Those segments just light up automatically when the relevant plugins are also installed (or, for the account segment, when you're logged in via Claude OAuth rather than an API key).

## Migrating from toolu ≤ 1.5.0

The statusline used to ship inside the `toolu` plugin and auto-symlinked to `~/.claude/toolu/statusline.sh`. It now lives here. To keep your statusline:

```text
/plugin install statusline@toolu
```

Then re-point `settings.json` from `~/.claude/toolu/statusline.sh` to `~/.claude/statusline/statusline.sh` — `/statusline:setup --force` does this for you (the old path is a custom value to it, so plain `/statusline:setup` would refuse).

## Hooks

| Hook | Event | Purpose |
|------|-------|--------|
| `session-start` | SessionStart | Symlinks the Bun renderer to `statusline/statusline.sh`; notices a `bash` statusLine that needs `/statusline:setup` |

## Testing

`bun test plugins/statusline` — real statusline JSON payloads, real git repos and published Jev wrappers, no mocks or API calls.
