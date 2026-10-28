# OpenCode

toolu runs under OpenCode V2 (probed on OpenCode 2.2). Its plugins keep their
Claude-format `hooks/hooks.json`, skills and commands; a generated JavaScript
shim connects them to OpenCode's plugin API, and the native binary does
everything else.

```bash
toolu plugins install --host opencode
```

The install copies each plugin into `~/.config/opencode/plugins/<name>/` and
writes its `server.js` ([installer](cli/installer.md#opencode)). OpenCode loads
the directories on its own, without a config change.

## How a call flows

OpenCode plugins can only be JavaScript, so `server.js` is the one JavaScript
toolu ships. It is filled in from `crates/core/protocol/src/launcher/opencode.js`
and holds no policy:

1. At load it finds `toolu` as the `hooks.json` launcher does (`TOOLU_BIN`
   alone when set, else the install directories, then `PATH`, keeping the first
   whose `--hook-protocol` prints an integer). It registers only the OpenCode
   hooks the plugin's `hooks.json` needs, and the plugin's skills and commands
   from `toolu opencode catalog`.
2. Each hook call goes to `toolu opencode bridge --event <hook> --plugin-root
   <dir>` as `{"directory", "event"}` on stdin.
3. The bridge translates the call into the Claude-shaped payload every toolu
   hook reads, runs the plugin's matching `hooks.json` entries as the binary's
   own `toolu [<plugin>] hook <name>` (with `TOOLU_HOST_OVERRIDE=opencode` and
   `CLAUDE_PLUGIN_ROOT`), and folds their output into one answer.
4. The shim acts on the answer: it throws to refuse a tool call, appends a
   post-tool block or advice to the tool's result, keeps session and prompt
   context in the session's instructions, and logs user messages to the server
   log.

| OpenCode | `hooks.json` event | Matcher subject |
|----------|--------------------|-----------------|
| first prompt or tool call of a session | `SessionStart` (`source: startup`) | source |
| `session.hook("prompt")` | `UserPromptSubmit` | none |
| `session.hook("compaction")` | `PreCompact` (`trigger: auto`) | trigger |
| `tool.hook("execute.before")` | `PreToolUse` | tool name |
| `tool.hook("execute.after")` | `PostToolUse` | tool name |

OpenCode has no `SessionEnd` or `PermissionRequest` hook toolu runs.

Tools take their Claude names, and their input gains Claude's keys beside
OpenCode's: `shell` is `Bash` (run in its `workdir`), `read`, `write` and `edit`
are `Read`, `Write` and `Edit` (`path` is also `file_path`), `patch` is
`apply_patch` (its `patchText` is `command`), and `glob`, `grep`, `webfetch`,
`websearch`, `skill`, `subagent` and `question` take their Claude names. A
completed call's `result.output` is the `tool_response`, with `exit_code` from
its `exit`. Any other tool keeps its name.

## Failure

- No usable `toolu`, a bridge that fails, or a hook that crashes or times out
  refuses a tool call and is logged on every other hook.
- A shim whose setup throws refuses every tool call, because OpenCode runs tools
  without a plugin whose setup throws.
- OpenCode cannot prompt from `execute.before`, so a gate's `ask` is a deny for
  a guardrail and advice for a judgement gate, as on Codex.

## Checking it

`toolu doctor --host opencode` lists toolu's copies and warns about one without
its `server.js`. Inside OpenCode's shell, `toolu` detects the host from
`OPENCODE` and `OPENCODE_SESSION_ID`.
