# statusline

Host-native project status. Claude Code gets an optional persistent one-line
statusline assembled defensively from the JSON Claude sends on stdin:

```
model | effort:high | ctx:45k/200k (22%) | example.com | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [JEV:READY]
```

| Segment | Source | Shows when |
|---------|--------|------------|
| model | `.model.display_name` | always |
| effort | `.effort.level` | the model reports an effort level |
| ctx | `.context_window.*` | always |
| `example.com` | `.oauthAccount.emailAddress` in `~/.claude.json` (or `$CLAUDE_CONFIG_DIR/.claude.json`) | logged in via Claude OAuth — shows only the email domain, not the full address |
| `✗ gate:failing` | host-native `.claude/tmp/quality-gate-status.json` at the git root | a **gate writer** (e.g. the `rust-quality` / `ts-quality` / `python-quality` / `toolu` plugins) marks the gate failing |
| folder + branch + status | git, from the workspace dir | inside a git repo — `↑N↓M` shows ahead/behind of the tracked remote, `[+N ~N ?N]` shows staged/unstaged/untracked file counts (both omitted when clean and up-to-date) |
| `[COMEMORY:N]` | `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/comemory-status/<repo>.json` | the **comemory** plugin published a memory count this session |
| `[JEV:READY]` / `[JEV:UNAVAILABLE: reason]` | `<config-dir>/jev/jev.sh`, Bun, and `TYPESAFE_API_KEY` in the environment | Jev published a wrapper; green when locally ready, yellow with the reason when unavailable |

Codex exposes `$statusline:status` instead of a persistent bar. It reports the
repository, branch/ahead/behind state, working-tree counts, quality gate from
`<repo>/.codex/tmp/quality-gate-status.json`, comemory count, and Jev readiness
(`Jev: ready` or `Jev: unavailable — reason`). It deliberately
omits account, model, effort, and context-window fields that Codex does not make
available to the skill.

Jev readiness uses the active host's config directory (`CLAUDE_CONFIG_DIR` or
`CODEX_HOME`, with `TOOLU_CONFIG_DIR` taking priority). It checks for an executable
wrapper, Bun, and a nonempty API key without line breaks. It never prints the
key, reads `.env`, executes the wrapper, or makes an API call. **Ready means local
prerequisites are present**; it does not verify authentication or service health.
An unpublished wrapper hides the segment; a broken published wrapper shows
`missing executable wrapper`. Other reasons include `missing bun`,
`missing TYPESAFE_API_KEY`, and `invalid TYPESAFE_API_KEY`. Multiple reasons are
separated by semicolons.

The account, gate, comemory, and git status segments degrade gracefully — if the file
they read is absent, the segment simply doesn't render. So statusline is
**standalone**: it declares no plugin dependencies. Those segments just light up
automatically when the relevant plugins are also installed (or, for the account
segment, when you're logged in via Claude OAuth rather than an API key).

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

## Codex install

```bash
codex plugin add statusline@toolu
```

Run `$statusline:status` whenever you want a current report. No setup or
persistent renderer is required.

## Claude Code install & wire up

Claude Code does not let a plugin declare `statusLine` in its manifest, so the
SessionStart hook symlinks the Bun renderer to a stable, version-independent
path:

```
~/.claude/statusline/statusline.sh   (→ the installed plugin's hooks/dist/statusline.js)
```

The link keeps its `.sh` name so existing `settings.json` entries stay valid,
but it is a Bun program: run it by path, never through `bash`.

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
       "command": "~/.claude/statusline/statusline.sh"
     }
   }
   ```

   (Use `"$CLAUDE_CONFIG_DIR/statusline/statusline.sh"` if you run with a custom
   config dir.) The symlink is refreshed every session, so plugin updates are
   picked up automatically with no settings change. The hook never clobbers a
   real file you place at that path — it only owns its own symlink.

## Migrating from the bash statusline

Before the Bun port, the wired command was `bash ~/.claude/statusline/statusline.sh`.
`bash` cannot run the Bun renderer, so the SessionStart hook prints a one-line
notice while `settings.json` still holds that value. Run `/statusline:setup`
once: it recognises the old command, backs `settings.json` up and rewrites it
to the path form above (no `--force` needed). jq and python3 are no longer
used.

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
