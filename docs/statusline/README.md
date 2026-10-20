# statusline

The statusline plugin uses the native `toolu` binary. Claude Code can display a
persistent one-line bar. Codex offers a status skill that runs the
native `toolu status` command.

## Install and set up Claude Code

Install `statusline@toolu`, then run `/statusline:setup`. It writes this setting
to Claude's `settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "toolu statusline render"
  }
}
```

Restart the session or run `/clear` so Claude loads the setting. `setup` keeps
other settings and saves `settings.json.bak` before changing an existing file.
It refuses to replace a custom `statusLine` unless you pass `--force`.
`toolu statusline setup --json` returns a status and settings path as JSON.

Claude sends a JSON payload to `toolu statusline render` on stdin. The command
prints the colored line without a trailing newline. `--json` returns
`{"line":"..."}` instead. Missing optional account, gate, comemory, or git
facts leave their segments out.

```text
model | effort:high | ctx:45k/200k (22%) | example.com | ✗ gate:failing | my-folder | main ↑2↓1 [+2 ~1 ?3] | [COMEMORY:42] | [JEV:READY]
```

The model, effort, and context come from Claude's payload. Only the domain of
the Claude OAuth email is shown. The repository root and branch come from
native `.git` readers. Ahead/behind and dirty counts come from a per-worktree
cache in the toolu config directory. The render never waits for Git: after a
missing or stale record it shows the last counts, if any, and starts one
detached refresh. A fresh record expires after two seconds or when HEAD or the
index changes. The gate comes from its local state file; comemory is read from
its local marker. Jev is
locally ready when `toolu` is executable on `PATH` and `TYPESAFE_API_KEY` is
set without line breaks. The renderer never sends a Jev API request or prints
the key.

## Existing settings

The native SessionStart hook no longer publishes
`<config root>/statusline/statusline.sh`. It rewrites a Claude `statusLine` that
ran that path, or its simple shell form, to `toolu statusline render`, saves a
backup, and shows one notice. When it cannot rewrite the setting, it says so and
leaves the path alone. Once nothing names the old path, it removes toolu's link
there; a user-owned regular file is kept. Later starts are silent. Run
`toolu statusline refresh` to apply a pending migration outside SessionStart.

A still older `~/.claude/toolu/statusline.sh` setting is treated as custom;
run `/statusline:setup --force` to replace it.

## Codex

Run `$statusline:status` in Codex. It runs `toolu --host codex status`.
The JSON document includes repository, working-tree, gate, push-review, and
waiver facts. It has no Claude model, context, or account fields.

## Muse

The statusline plugin is not supported on Muse: it ships no `.muse-plugin/plugin.json` yet (#547), so there is no Muse statusline or setup.

## Verification

The native crate and CLI tests use real repositories, settings files, linked
worktrees, and hook processes. `crates/cli/tests/golden_statusline.rs` replays
the render, setup and SessionStart cases in `fixtures/statusline/cases.json`
against the binary. The render budget is measured with
`cargo xtask bench-statusline --assert`.
