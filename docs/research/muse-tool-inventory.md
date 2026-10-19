# Muse tool inventory and hook verdicts (Muse Code 1.4.10)

> Date: 2026-10-19 · Issue: #540 (epic #539) · Host: Muse Code `1.4.10-R6427.1`, model `muse-spark-1.3-contributor` (meta) · Machine-readable evidence: [`docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`](../toolu/evidence/2026-10-19-muse-tool-inventory.json)

## TL;DR

- **Tool names.** Muse exposes 39 tool names across the configurations a toolu user can run. Hooks saw 33 of them in live sessions, plus 2 internal tools. Four were not exercised:
  - `write_todos` was called by the model, but Muse never fires a hook for it;
  - `snooze_reminder` had nothing to snooze;
  - `list_peer_sessions` and `send_session_message` are never offered to a `muse exec` session.
- **Configuration changes the toolset.**
  - Trusting the workspace adds six `subagent_*` tools.
  - `--enable-shell-tool` swaps `bash` and `bash_input` for `shell`.
  - `--preset native-basic` changes nothing in `muse exec`.
  - MCP tools arrive as `mcp__plugin_<plugin>_<server>__<tool>`.
- **Ask: there is an ask surface.** On `PreToolUse`, `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask",…}}` forces a human approval prompt under the default `--approval-mode on-request`. It bypasses the LLM approval judge, and a headless `muse exec` then waits forever. Under `untrusted` the request is folded into the judge-decided approval; under `never` it is ignored. `PermissionRequest` has no ask.
- **Input rewrite: supported.** `hookSpecificOutput.updatedInput` on `PreToolUse` replaces the whole `tool_input`, and the rewritten command runs.
- **PostToolUse block: replaces the tool result.** The model receives only the block reason. The original result never reaches it, and the edit stays on disk. Exit-0 `{"decision":"block"}` and exit-2 stderr behave the same.
- **Hook gaps:**
  - failed calls, including a `bash` command with a non-zero exit, go to `PostToolUseFailure`, not `PostToolUse`;
  - `PermissionRequest` fires only on some approval paths, and after the LLM judge has already decided;
  - a `web_fetch` network approval fires no `PermissionRequest`.
- **Corrections to the epic's measured contract (#539).** The epic recorded:
  - "Muse shows no ask surface";
  - "`updated_input` stays unsupported";
  - "no hook output sets `permission_decision`";
  - "`hookSpecificOutput` shapes → failed".

  For `PreToolUse` and `PermissionRequest`, all four are wrong. `hookSpecificOutput` is the supported shape there: it carries `permissionDecision` deny/ask (allow only together with `updatedInput`), `updatedInput`, `additionalContext` (hook test only), and `decision.behavior` allow/deny on `PermissionRequest`.

## Method

The probe is a temporary native plugin, `toolu-probe-540`. Each tool-relevant event has its own logging hook, because Muse forbids two hooks sharing a source file. A stdio MCP server, `probe-mcp`, offers one tool, `probe_echo {text}`. `muse plugins validate` reported it valid with zero diagnostics. Every hook appends its stdin payload and its `MUSE_`/`CLAUDE_`/`PLUGIN_` environment to a log. It then answers from a control file whose first matching rule (`event`, optional `tool`, optional `match` substring of `tool_input`) supplies stdout, stderr and the exit code. Without a matching rule it exits 0 silently.

```text
.muse-plugin/plugin.json (abridged)
{ "schemaVersion": 1, "name": "toolu-probe-540", "version": "0.1.0",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "hooks": [
      { "id": "pre-tool-use",          "event": "PreToolUse",         "command": ["python3", "hooks/pre_tool_use.py"],          "timeoutMs": 10000 },
      { "id": "post-tool-use",         "event": "PostToolUse",        "command": ["python3", "hooks/post_tool_use.py"],         "timeoutMs": 10000 },
      { "id": "post-tool-use-failure", "event": "PostToolUseFailure", "command": ["python3", "hooks/post_tool_use_failure.py"], "timeoutMs": 10000 },
      { "id": "permission-request",    "event": "PermissionRequest",  "command": ["python3", "hooks/permission_request.py"],    "timeoutMs": 10000 },
      { "id": "subagent-stop",         "event": "SubagentStop",       "command": ["python3", "hooks/subagent_stop.py"],         "timeoutMs": 10000 },
      { "id": "notification",          "event": "Notification",       "command": ["python3", "hooks/notification.py"],          "timeoutMs": 10000 },
      { "id": "session-start",         "event": "SessionStart",       "command": ["python3", "hooks/session_start.py"],         "timeoutMs": 10000 } ],
    "mcpServers": [ { "id": "probe-mcp", "transport": "stdio", "command": ["python3", "mcp/server.py"] } ],
    "skills": [], "commands": [], "reminders": [] } }

hooks/<event>.py:  sys.exit(probe_lib.main("<Event>"))

hooks/probe_lib.py (core):
  raw = sys.stdin.read(); payload = json.loads(raw)
  rule = first rule in control.json whose event matches, whose "match" (if any) is a substring
         of json.dumps(payload["tool_input"]), and whose "tool" (if any) equals payload["tool_name"]
  append {configured_event, run_tag, argv, cwd, env (MUSE_/CLAUDE_/PLUGIN_/CODEX_/CURSOR_), payload, rule} to hooks.jsonl
  write rule.stdout / rule.stderr, exit rule.exit (0 without a rule)
```

Every run used one environment:

- `XDG_DATA_HOME=<scratch>/xdg-data`, which kept the plugin store, the plugin cache and the session logs out of the global store shared with the parallel #541 worker. `--scope project` does not isolate, because it still writes the global lockfile.
- `MUSE_UPDATE_INTERVAL_SECONDS=99999999`, which holds the launcher on 1.4.10. Every session log records `build.semver` `1.4.10`.
- `cwd` set to a one-file git workspace (`<ws>`).
- The global `~/.config/muse/auth.json`, used in place and never copied or read.

```text
muse plugins install <scratch>/probe --json          # with XDG_DATA_HOME set
muse plugins approve toolu-probe-540 --json          # writes trust keys to ~/.config/muse/settings.json
muse exec --json <flags> "<prompt naming the exact tool>" < /dev/null
muse plugins hook test toolu-probe-540:<hook-id> --fixture <{"event":"<Event>","stdin":<payload>}> --json
muse plugins remove toolu-probe-540 --delete-data --json
```

Each run gives up to five views:

1. the hook log, which is what a hook sees;
2. the session log's `model_request_configured.toolset.active_tools`, the model-visible toolset;
3. `assistant_tool_calls_committed`, what the model called;
4. `tool_result_batch_committed`, the exact tool-result text the model received;
5. the records between that result and the next assistant message.

The runs:

| Runs | Flags | Purpose |
|------|-------|---------|
| I0 | `--approval-mode never` | untrusted default toolset |
| I1–I11, I9b | `--approval-mode never --trust-workspace` | one tool family per run: file tools; search, skill and todos; web; memory; cron, `monitor` and `bash_input`; goals and work; image; MCP; workflow and subagents (I9b passes `host.agent` options); work tools and every `subagent_spawn` parameter; failing calls |
| P1, S1 | `--preset native-basic` or `--enable-shell-tool`, with I1's flags | alternative surfaces |
| A0 | `--approval-mode untrusted --trust-workspace` | approval calibration (`touch`, no hook rule) |
| A1–A11, AP1, AS1 | I1–I11, P1 and S1 prompts under `--approval-mode untrusted --trust-workspace` | which tools fire `PermissionRequest` |
| V-ask-* | four ask shapes × `on-request`, `untrusted` and `never`, plus deny and allow under `untrusted` | ask verdict |
| V-rw-* | four `updatedInput` shapes, `never` | input-rewrite verdict |
| V-post-* | exit-0 JSON block and exit-2 stderr on `edit_file`, `never` | PostToolUse-block verdict |

**Cross-check.** Every `assistant_tool_calls_committed` call in the main sessions of I0–I11, P1 and S1 was matched against a `PreToolUse` hook line. Only `write_todos` (I2) had none: the call ran and returned `{"items":2,"ok":true,"revision":1}`, but no `PreToolUse`, `PostToolUse` or `PostToolUseFailure` hook fired.

Event payloads, per event (union of keys over every observed call):

| Event | Payload keys |
|-------|--------------|
| `Notification` | `cwd`, `hook_event_name`, `message`, `model`, `notification_type`, `permission_mode`, `session_id`, `title`, `transcript_path`, `turn_id` |
| `PermissionRequest` | `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `tool_input`, `tool_name`, `transcript_path`, `turn_id` |
| `PostToolUse` | `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `tool_input`, `tool_name`, `tool_response`, `tool_use_id`, `transcript_path`, `turn_id` |
| `PostToolUseFailure` | `cwd`, `duration_ms`, `error`, `hook_event_name`, `is_interrupt`, `model`, `permission_mode`, `session_id`, `tool_input`, `tool_name`, `tool_use_id`, `transcript_path`, `turn_id` |
| `PreToolUse` | `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `tool_input`, `tool_name`, `tool_use_id`, `transcript_path`, `turn_id` |
| `SessionStart` | `cwd`, `hook_event_name`, `model`, `permission_mode`, `session_id`, `source`, `transcript_path` |
| `SubagentStop` | `child_session_id`, `cwd`, `hook_event_name`, `last_assistant_message`, `model`, `permission_mode`, `session_id`, `stop_hook_active`, `subagent_id`, `transcript_path`, `turn_id` |

- `transcript_path` is `null` on every event in `muse exec`.
- `permission_mode` is `bypassPermissions` under `--approval-mode never` and `default` under `on-request` and `untrusted`.
- `PermissionRequest` carries no `tool_use_id`.
- `PostToolUseFailure` replaces `tool_response` with `error`, `duration_ms` and `is_interrupt`.
- `SessionStart` `source` was `startup` in every session. All 54 sandbox session logs record `build.semver` `1.4.10`.
- Hooks get `MUSE_PLUGIN_ID`, `MUSE_PLUGIN_ROOT`, `MUSE_PLUGIN_DATA_DIR`, `MUSE_SESSION_ID`, `MUSE_THREAD_ID`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, `PLUGIN_ROOT` and `PLUGIN_DATA`. `muse plugins hook test` sets neither `MUSE_SESSION_ID` nor `MUSE_THREAD_ID`.
- A relative source argument in the hook argv (`hooks/pre_tool_use.py`) reaches the process as the absolute path inside the cached package.

## Toolsets

| Configuration | Flags | Count | Difference from `default-untrusted` |
|---------------|-------|-------|--------------------------------------|
| `default-untrusted` | `--approval-mode never` | 28 | baseline |
| `default-trusted` | `--approval-mode never --trust-workspace` | 34 | + `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_wait`, `subagent_read_result`, `subagent_cancel` |
| `native-basic` | `--preset native-basic --approval-mode never --trust-workspace` | 34 | + `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_wait`, `subagent_read_result`, `subagent_cancel` |
| `enable-shell-tool` | `--enable-shell-tool --approval-mode never --trust-workspace` | 33 | + `shell`, `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_wait`, `subagent_read_result`, `subagent_cancel`; − `bash`, `bash_input` |

`default-untrusted` lists, in Muse's order: `workflow`, `read_file`, `search`, `write_file`, `edit_file`, `read_memory`, `add_memory`, `edit_memory`, `work_stop`, `work_list`, `web_fetch`, `web_search`, `bash`, `bash_input`, `monitor`, `cron_create`, `cron_delete`, `cron_list`, `get_goal`, `create_goal`, `update_goal`, `report_progress`, `image_generation`, `read_skill`, `work_status`, `mcp__plugin_toolu_probe_540_probe_mcp__probe_echo`, `snooze_reminder`, `write_todos`.

- **Child agents** have their own toolsets:
  - a `subagent_spawn` child has 22 tools: the trusted main set without `workflow`, `work_list`, `work_stop`, the cron and goal tools, `snooze_reminder` and `write_todos`;
  - a workflow `host.agent` child has 17 tools: it also lacks the `subagent_*` tools, and adds `submit_result`;
  - every reminder agent has `submit_reminder_decision` only (`mode: named`, `source: runtime`).
- **`image_generation`** is offered only with the `meta` provider: the `echo` provider's toolset lacks it.
- **Feature config.** Seeding the sandbox with the global feature config (`gates.local_session_messaging: true`) changed no toolset.

## Tool inventory

"Exercised" means a `PreToolUse` hook saw the call. The keys are the union over every observed call. The `PermissionRequest` column comes from runs A1–A11, AP1 and AS1:

- `fired` means a `PermissionRequest` hook line with that `tool_name` appeared;
- `not fired` means the tool ran under `untrusted` without one.

The proposed canonical names follow the engine's existing names (`Bash`, `Read`, `Write`, `Edit`, `Grep`, `WebFetch`, `WebSearch`, `Skill`, `Agent`, …) for #543.

| Tool | Family | `tool_input` keys | Post event and `tool_response` | `PermissionRequest` under `untrusted` | Proposed canonical |
|------|--------|-------------------|--------------------------------|------------------|--------------------|
| `bash` | shell | `command`, `description`, `tty`, `workdir`, `yield_time_ms` | PostToolUse: string (JSON text, object with keys chunk_id, command, description, exit_code, original_output_bytes, original_output_tokens, output, terminal_status, truncated); failures go to `PostToolUseFailure` | fired | Bash |
| `bash_input` | shell | `chars`, `session_id`, `yield_time_ms` | PostToolUse: string (JSON text, object with keys chunk_id, command, description, exit_code, original_output_bytes, original_output_tokens, output, terminal_status, truncated) | not fired | none: stdin to a running bash job |
| `shell` | shell | `command` | PostToolUse: string (plain text) | fired | Bash |
| `read_file` | file-read | `path` | PostToolUse: string (plain text); failures go to `PostToolUseFailure` | not fired | Read |
| `write_file` | file-write | `content`, `path` | PostToolUse: object with keys filePath, structuredPatch | not fired | Write |
| `edit_file` | file-edit | `find`, `path`, `replace` | PostToolUse: object with keys filePath, structuredPatch | not fired | Edit |
| `search` | search | `glob`, `mode`, `output_mode`, `pattern` | PostToolUse: string (plain text) | not fired | Grep (glob is a file-filter array; output_mode takes Grep values) |
| `web_fetch` | web | `url` | PostToolUse: string (plain text) | not fired | WebFetch |
| `web_search` | web | `query` | PostToolUse: string (JSON text, object with keys query, results) | not fired | WebSearch |
| `read_skill` | skill | `name` | PostToolUse: string (plain text) | not fired | Skill |
| `add_memory` | memory | `content`, `path` | PostToolUse: string (JSON text, object with keys message, operation, path, scope, success) | not fired | none |
| `edit_memory` | memory | `new_str`, `old_str`, `path` | PostToolUse: string (JSON text, object with keys message, operation, path, scope, success) | not fired | none |
| `read_memory` | memory | `path` | PostToolUse: string (JSON text, object with keys content, path, scope, start_line_number, success, truncated) | not fired | none |
| `cron_create` | cron | `cron`, `prompt` | PostToolUse: string (plain text) | not fired | CronCreate |
| `cron_delete` | cron | `id` | PostToolUse: string (plain text) | not fired | CronDelete |
| `cron_list` | cron | none (`{}`) | PostToolUse: string (JSON text, object with keys jobs) | not fired | CronList |
| `monitor` | monitor | `command`, `description` | PostToolUse: string (JSON text, object with keys message, persistent, timeout_ms, work_id) | fired | Monitor |
| `workflow` | workflow | `name`, `script` | PostToolUse: string (JSON text, object with keys args, entryId, hostApiVersion, maxParallelAgents, message, policy, scriptBytes, scriptHash, scriptPath, status, taskId, tokenBudget, workId, workflowRunId, workflowRunIdDescription) | not fired | Workflow |
| `subagent_cancel` | delegation | `command_id`, `subagent_id` | PostToolUse: string (JSON text, object with keys status, subagent_id, task_ref) | fired | TaskStop |
| `subagent_read_result` | delegation | `subagent_id` | PostToolUse: string (JSON text, object with keys evidence_refs, status, subagent_id, summary, task_ref, workspace) | fired | none |
| `subagent_send_message` | delegation | `command_id`, `message`, `subagent_id` | PostToolUse: string (JSON text, object with keys reason, status, subagent_id) | fired | SendMessage |
| `subagent_spawn` | delegation | `command_id`, `context_policy_ref`, `objective`, `output_schema`, `role`, `subagent_type`, `task_name`, `worktree_isolation` | PostToolUse: string (JSON text, object with keys agent_path, status, subagent_id, task_ref, work_id) | fired | Agent |
| `subagent_status` | delegation | `subagent_id` | PostToolUse: string (JSON text, object with keys roster, status, subagent_id, task_ref, workspace) | fired | none |
| `subagent_wait` | delegation | `command_id`, `subagent_id`, `timeout_ms`, `wait_for` | PostToolUse: string (JSON text, object with keys evidence_refs, status, subagent_id, summary, task_ref, workspace) | fired | none |
| `create_goal` | goal | `objective` | PostToolUse: string (JSON text, object with keys goal) | not fired | none |
| `get_goal` | goal | none (`{}`) | PostToolUse: string (JSON text, object with keys goal) | not fired | none |
| `report_progress` | goal | `current_work`, `next_work`, `percent_complete` | PostToolUse: string (JSON text, object with keys goal) | not fired | none |
| `update_goal` | goal | `status` | PostToolUse: string (JSON text, object with keys goal) | not fired | none |
| `work_list` | work | none (`{}`) | PostToolUse: string (JSON text, object with keys items, next_after_work_id) | not fired | none |
| `work_status` | work | `work_id` | PostToolUse: string (JSON text, object with keys artifacts, controls, detail, family, inspection_hint, lifecycle, status, terminal, work_id) | not fired | none |
| `work_stop` | work | `work_id` | PostToolUse: string (JSON text, object with keys request_id, status, work_id) | not fired | TaskStop |
| `image_generation` | image | `prompt` | PostToolUse: string (plain text) | fired | none |
| `write_todos` | todo | — | not exercised: the model called it (run I2; result {"items":2,"ok":true,"revision":1}) but Muse fired no PreToolUse, PostToolUse or PostToolUseFailure hook for it: hook coverage gap | not exercised | TodoWrite |
| `snooze_reminder` | reminder | — | not exercised: listed in every default toolset, but no snoozable reminder existed in any run (I6 prompt skipped it) | not exercised | none |
| `list_peer_sessions` | peer-message | — | not exercised: never offered to a muse exec session: absent from the default, trusted, native-basic, enable-shell-tool and echo toolsets even with gates.local_session_messaging=true; the binary lists it only in the native-basic profile table | not exercised | none |
| `send_session_message` | peer-message | — | not exercised: never offered to a muse exec session (same toolsets as list_peer_sessions) | not exercised | SendMessage |
| `mcp__plugin_toolu_probe_540_probe_mcp__probe_echo` | mcp | `text` | PostToolUse: string (plain text) | fired | mcp__plugin_toolu_probe_540_probe_mcp__probe_echo |
| `submit_reminder_decision` | internal | `additionalProperties`, `advisory_text`, `confidence`, `decision`, `next_step`, `priority`, `reason`, `skill_id`, `visible_for_steps` | PostToolUse: string (plain text); failures go to `PostToolUseFailure` | not fired | ignore |
| `submit_result` | internal | `notes`, `reply`, `text` | PostToolUse: string (plain text) | fired | ignore |

## Examples

One redacted example per exercised family:

- ids become `<id>`;
- the probe workspace becomes `<ws>`;
- strings over 200 characters are truncated with `…`.

The complete `pre_example` and `post_example` of every tool are in the evidence JSON.

### Example — shell

`bash`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "bash",
  "tool_input": {
    "command": "read line; echo got:$line",
    "description": "Wait for probe input",
    "workdir": "<ws>"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\n  \"chunk_id\": \"exec-1-1\",\n  \"command\": \"read line; echo got:$line\",\n  \"description\": \"Wait for probe input\",\n  \"exit_code\": 0,\n  \"terminal_status\": \"completed\",\n  \"output\": \"got:\\n\",\n  \"original_out…"
```

### Example — file-read

`read_file`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "read_file",
  "tool_input": {
    "path": "probe-new.txt"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"Read text file `probe-new.txt`.\n1|three"
```

### Example — file-write

`write_file`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "write_file",
  "tool_input": {
    "content": "one",
    "path": "probe-new.txt"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
{"filePath": "<ws>/probe-new.txt", "structuredPatch": [{"lines": ["+one"], "newLines": 1, "newStart": 1, "oldLines": 0, "oldStart": 1}]}
```

### Example — file-edit

`edit_file`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "edit_file",
  "tool_input": {
    "find": "two",
    "path": "probe-new.txt",
    "replace": "three"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
{"filePath": "<ws>/probe-new.txt", "structuredPatch": [{"lines": ["-two", "+three"], "newLines": 1, "newStart": 1, "oldLines": 1, "oldStart": 1}]}
```

### Example — search

`search`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "search",
  "tool_input": {
    "pattern": "alpha"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"notes.txt:1:alpha"
```

### Example — web

`web_fetch`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "web_fetch",
  "tool_input": {
    "url": "https://example.com"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"Example Domain\n\nThis domain is for use in documentation examples without needing permission. This is not a service; avoid relying on it for testing and monitoring purposes.\n\nSource: https://example.co…"
```

### Example — skill

`read_skill`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "read_skill",
  "tool_input": {
    "name": "bundled:blender"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"<read-skill-result name=\"bundled:blender\" status=\"ok\">\n<metadata>\npath: <scratch>/xdg-data/muse/skills/bundled/muse-core/skills/blender/SKILL.md\nlocator: file://<scratch>/xdg-data/muse/skills/bundled/…"
```

### Example — memory

`add_memory`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "add_memory",
  "tool_input": {
    "content": "probe-540 memory note",
    "path": "probe-540.md"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"success\":true,\"scope\":\"personal_project\",\"path\":\"probe-540.md\",\"operation\":\"add\",\"message\":\"memory note written\"}"
```

### Example — cron

`cron_create`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "cron_create",
  "tool_input": {
    "cron": "0 0 1 1 *",
    "prompt": "say probe-540"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"Scheduled f1198916 (0 0 1 1 *)"
```

### Example — monitor

`monitor`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "monitor",
  "tool_input": {
    "command": "sleep 2; echo monitor-540",
    "description": "Monitor probe command"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"work_id\":\"work.v2.monitor.sha256.<hash>\",\"timeout_ms\":300000,\"persistent\":false,\"message\":\"Monitor started (work work.v2.monitor.sha256.<hash>, timeout 5m). Events will be delivered automatically. K…"
```

### Example — workflow

`workflow`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "workflow",
  "tool_input": {
    "name": "child-540 echo",
    "script": "export default async function workflow(host) { const result = await host.agent({ input: \"Reply with the text child-540\" }); return { status: \"ok\", ref: result.ref, text: result.text }; }"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"args\":{},\"entryId\":\"child-540 echo\",\"hostApiVersion\":\"v1\",\"maxParallelAgents\":6,\"message\":\"Workflow launched: the workflow runs in the background and its completion is delivered to this conversation…"
```

### Example — delegation

`subagent_cancel`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "subagent_cancel",
  "tool_input": {
    "command_id": "cancel-540-1",
    "subagent_id": "<id>"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"status\":\"already_terminal\",\"subagent_id\":\"<id>\",\"task_ref\":\"task/<id>#5\"}"
```

### Example — goal

`create_goal`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "create_goal",
  "tool_input": {
    "objective": "probe-540 goal: write the word done"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"goal\":{\"created_at_ms\":1792423478096,\"current_work\":null,\"goal_id\":\"goal-<id>\",\"last_progress_at_ms\":null,\"next_work\":null,\"objective\":\"probe-540 goal: write the word done\",\"percent_complete\":0,\"ses…"
```

### Example — work

`work_list`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "work_list",
  "tool_input": {},
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"{\"items\":[],\"next_after_work_id\":null}"
```

### Example — image

`image_generation`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "image_generation",
  "tool_input": {
    "prompt": "tiny 64x64 image of a plain red square, solid red fill, no border, no background, no other elements"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"Saved generated image to generated_images/call_<id>.png"
```

### Example — mcp

`mcp__plugin_toolu_probe_540_probe_mcp__probe_echo`, redacted `PreToolUse` payload (common keys in the event table above):

```json
{
  "hook_event_name": "PreToolUse",
  "tool_name": "mcp__plugin_toolu_probe_540_probe_mcp__probe_echo",
  "tool_input": {
    "text": "hello-540"
  },
  "permission_mode": "bypassPermissions"
}
```

`PostToolUse` `tool_response`:

```json
"probe-echo: hello-540"
```

## Internal tools

These belong on the normalize ignore list. Neither is in any main-agent toolset, and both run inside an agent Muse starts for its own purposes.

- **`submit_reminder_decision`.** Muse starts reminder agents (`skill-reminder`, `goal-reminder`, `verify-reminder`) on every run, without being asked.
  - Each runs with the toolset `{mode: named, source: runtime, active_tools: [submit_reminder_decision]}`.
  - The hook payload's `session_id` is the reminder agent's own session, not the user's.
  - Each reminder adds three hook invocations: `PreToolUse`, `PostToolUse` or `PostToolUseFailure`, and `SubagentStop`. The 13 A-runs alone logged 29 `submit_reminder_decision` calls.
- **`submit_result`.** A workflow child started by `host.agent(...)` returns its answer with this tool. Its toolset (`mode: all`, 17 tools) ends with `submit_result`, which no main toolset contains. Under `untrusted` it even fires `PermissionRequest`.

`SubagentStop` fires for every reminder agent, workflow child and `subagent_spawn` child. Its payload has `child_session_id`, `subagent_id`, `last_assistant_message` and `stop_hook_active`.

## Ask verdict

**Verdict: Muse has an ask surface, on `PreToolUse` only.** The supported shape is `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"…"}}` with exit 0. What it does depends on the approval mode:

- **`on-request` (the default):** it raises a human approval prompt.
- **`untrusted`:** it merges into the mode's own approval, which the LLM judge decides.
- **`never`:** it is ignored.

`PermissionRequest` has no ask value.

**Calibration (A0).** This shows what a normal approval looks like headless. A `bash` `touch` with no hook rule under `untrusted` produced these records:

- `requested` with `presentation_phase: automated_reviewing` and the choices allow once, always allow `touch …` in this workspace, and reject;
- `approval_wait.effect.started`;
- `automated_review_started` and `automated_review_completed` (the LLM approval judge, `reasoning_effort: low`, outcome approve);
- `decision_applied` with `decision_source.kind: llm_judge`;
- the `PermissionRequest` hook;
- `approval_wait.effect.terminal` approved.

The `PermissionRequest` hook therefore runs after the judge has decided.

**The hook-forced prompt (V-ask-c-o).** The `hookSpecificOutput` ask shape under `on-request` produced a different sequence:

1. `requested` with `presentation_phase: human_pending`, the choices allow once and abort, and empty `stages`.
2. `approval_wait.effect.started` with no automated review.
3. The `PermissionRequest` hook, whose own output failed.
4. A `Notification` hook with `notification_type: permission_prompt`, `title: "Muse Code needs approval"` and `message: "ws — waiting for approval"`.

Nothing followed. The headless session waited until the harness killed it after 900 s, and the file was never created.

The same `touch` under `on-request` with any other hook output raised no approval at all. So the prompt came from the hook, not the mode. `muse plugins hook test` under-reports this shape: it shows `status=completed` with `permission_decision: null`, and only the live runtime raises the prompt.

| Event | Hook output | `hook test` | `on-request` | `untrusted` | `never` |
|-------|-------------|-------------|--------------|-------------|---------|
| PreToolUse | `{"decision":"ask","reason":"probe-540 ask"}` | status=failed; error=unsupported legacy PreToolUse output; use `hookSpecificOutput.permissionDecision` | failed | failed | failed |
| PermissionRequest | `{"decision":"ask","reason":"probe-540 ask"}` | status=failed; error=unsupported `decision.behavior` value in PermissionRequest output | not-invoked | failed | not-invoked |
| PreToolUse | `{"permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}` | status=failed; error=unsupported `permissionDecision` in output of PreToolUse hook output | failed | failed | failed |
| PermissionRequest | `{"permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}` | status=failed; error=unsupported `permissionDecision` in output of PermissionRequest hook output | not-invoked | failed | not-invoked |
| PreToolUse | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}}` | status=completed | prompted | ran | ran |
| PermissionRequest | `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}}` | status=failed; error=unsupported `permissionDecision` in hookSpecificOutput of PermissionRequest hook output | failed | failed | not-invoked |
| PreToolUse | `{"decision":"approve"}` | status=failed; error=unsupported legacy PreToolUse output; use `hookSpecificOutput.permissionDecision` | failed | failed | failed |
| PermissionRequest | `{"decision":"approve"}` | status=failed; error=unsupported `decision.behavior` value in PermissionRequest output | not-invoked | failed | not-invoked |

| Event | Decision | Hook output | `hook test` | Live under `untrusted` | Tool result |
|-------|----------|-------------|-------------|------------------------|-------------|
| PreToolUse | deny | `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"probe-540 deny"}}` | status=blocked; parsed={"permission_decision": "deny", "should_block": true} | blocked | `tool blocked by hook: probe-540 deny` |
| PermissionRequest | deny | `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"deny","message":"probe-540 deny"}}}` | status=blocked; parsed={"permission_decision": "deny", "should_block": true} | blocked | `tool denied by hook: probe-540 deny` |
| PermissionRequest | allow | `{"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow"}}}` | status=completed; parsed={"permission_decision": "allow"} | ran | `{ "chunk_id": "exec-1-1", "command": "touch ask540-g-u.txt", "description": "Create empty ` |

The outcome values are:

- `prompted`: a human-pending approval appeared that the mode does not raise by itself;
- `ran`: the tool ran and the ask had no effect;
- `failed`: Muse rejected the hook output and the tool ran;
- `not-invoked`: the event did not fire, because the mode raised no approval.

Under `untrusted`, `ran` for the `hookSpecificOutput` shape means the request went through the same judge-approved records as A0.

**Consequences for #544:**

- **`supports_ask(Muse, PreToolUse)` is true, with a caveat.** In a headless `muse exec` the ask never resolves.
- **Allow requires `updatedInput`.** A bare `permissionDecision: "allow"` is rejected (`requires updatedInput; a bare allow is rejected`).
- **The legacy shapes fail open.** `{"decision":"ask"}`, top-level `permissionDecision` and `{"decision":"approve"}` are rejected as failed hooks and the tool runs. The epic's live run showed `{"decision":"block","reason":…}` still blocks. `ask` and `approve` are the legacy values Muse refuses, with the message "use `hookSpecificOutput.permissionDecision`".
- **Deny is honoured on both events**, with different texts: "tool blocked by hook: …" on `PreToolUse`, and "tool denied by hook: …" on `PermissionRequest` (`decision.behavior: "deny"`; a hook test also accepted it without the `hookSpecificOutput` wrapper).
- **`PermissionRequest` `decision.behavior: "allow"`** let the call run with no approval record at all.

## Input-rewrite verdict

**Verdict: supported, through `hookSpecificOutput.updatedInput` on `PreToolUse`.** It works on its own or together with `permissionDecision: "allow"`, with exit 0.

- **It replaces the whole `tool_input`.** In V-rw-r2 and V-rw-r4 the model called `echo original-540-r*` with a `description`. Bash ran `echo rewritten-540-r*` and returned a result with no `description` key, so every key the hook omits is dropped.
- **Top-level `updatedInput` fails.** With or without `decision: "approve"`, the hook fails and the original command runs.
- **The hook-test fixture used a different command.** It rewrote to `echo rewritten-540` without the run suffix.

| Hook output | `hook test` | Live hook status | Ran | Tool result |
|-------------|-------------|------------------|-----|-------------|
| `{"updatedInput":{"command":"echo rewritten-540-r1"}}` | status=failed; error=unsupported `updatedInput` in output of PreToolUse hook output | completed | original-540 | command `echo original-540-r1`, output `original-540-r1` |
| `{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"echo rewritten-540-r2"}}}` | status=completed; parsed={"updated_input": {"command": "echo rewritten-540"}} | completed | rewritten-540 | command `echo rewritten-540-r2`, output `rewritten-540-r2` |
| `{"decision":"approve","updatedInput":{"command":"echo rewritten-540-r3"}}` | status=failed; error=unsupported `updatedInput` in output of PreToolUse hook output | completed | original-540 | command `echo original-540-r3`, output `original-540-r3` |
| `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"command":"echo rewritten-540-r4"}}}` | status=completed; parsed={"updated_input": {"command": "echo rewritten-540"}} | completed | rewritten-540 | command `echo rewritten-540-r4`, output `rewritten-540-r4` |

## PostToolUse-block verdict

**Verdict: a `PostToolUse` block replaces the tool result the model receives with the block reason.** In both runs:

- the model received exactly `probe-540 post block` for its `edit_file` call, where an unblocked call returns the `{filePath, structuredPatch}` object;
- the model quoted the reason as the tool's output;
- the records up to the next assistant message were only reminder-agent and model-request bookkeeping. No developer or system message repeats the reason, and the original result never reaches the model;
- the edit stayed on disk, because the hook runs after the tool.

Exit 0 with `{"decision":"block","reason":…}` and exit 2 with the reason on stderr behave identically. The model's quote below is shown with Markdown backticks removed.

| Hook output | Exit | `hook test` | Tool result the model received | Model's verbatim quote | Edit on disk |
|-------------|------|-------------|-------------------------------|------------------------|--------------|
| `{"decision":"block","reason":"probe-540 post block"}` | 0 | status=blocked; parsed={"should_block": true} | `probe-540 post block` | `The edit_file result was: probe-540 post block` | persisted |
| stderr `probe-540 post block` | 2 | status=blocked; parsed={"should_block": true} | `probe-540 post block` | `probe-540 post block` | persisted |

## Side effects and cleanup

`muse plugins remove toolu-probe-540 --delete-data` ran with the sandbox `XDG_DATA_HOME`. It also removed the eight `plugin:toolu-probe-540:*` trust keys that `muse plugins approve` had written to the global `~/.config/muse/settings.json`. That file is back to its 82-byte pre-probe content. `muse plugins marketplace list` shows no marketplace in the sandbox, and none named for this probe globally.

The plugin lists right after removal, verbatim:

```text
sandbox:  no plugins
global:   brainstorm	8.0.3	enabled=true active=true trust=user-local provenance=marketplace-user-added valid=true diagnostics=7
```

The global entry is not the probe's. `brainstorm` was installed at 15:10:48Z from the user-added `pin-probe` marketplace, one of the probe marketplaces the parallel #541 marketplace spike registered in the machine's shared global Muse store. The probe itself never touched the global plugin store. A file snapshot of `~/.config/muse`, `~/.local/share/muse` (sessions and indexes excluded), `~/.agents` and `~/.muse`, taken before and after, shows only two changes the probe caused:

| Path | Cause | Removed | Reason |
|------|-------|---------|--------|
| `~/.config/muse/settings.json` | muse plugins approve toolu-probe-540 wrote 8 runtime_capabilities trust keys (plugin:toolu-probe-540:*); approval trust is global even with XDG_DATA_HOME set | no | the file is the user's Muse settings and stays; muse plugins remove deleted the probe's 8 keys, the file is back to its 82-byte pre-probe content, and S6 checks that no plugin:toolu-probe-540 key remains |
| `~/.local/share/muse/session-name-authority/session-names.db` | every muse exec session claims an automatic session name in this machine-wide registry, which XDG_DATA_HOME does not relocate; 53 of its 69 claims belong to probe sessions | no | shared SQLite registry that every Muse session on the machine writes, the parallel #541 worker's included; deleting rows from another program's live database risks corrupting it, and the claims only reserve names of sessions that exist solely in the probe's sandbox data home |

Everything else the runs created stays inside the session scratchpad: session logs, cron jobs (`cron.db` per session, deleted by `cron_delete` in I5), goals, the memory note (`memory/projects/…/probe-540.md` in the sandbox data home, emptied by `edit_memory`), generated images and marker files. Other snapshot changes (`muse-probe-a`, `probe-codex`, `installed.json`, `marketplaces.json`) belong to the #541 worker.

## Notes for #543, #544 and #554

**#543: payload, normalize and canonical names**

- **Tool names.** Map `bash` and `shell` to `Bash`: both carry `command`, which the engine's shell detection already reads. `bash_input` (stdin to a running job) and the `*_goal`, `work_*`, memory and image tools have no Claude equivalent, so they should stay as tool-level events. Proposed names are in the inventory table.
- **Input keys differ from Claude's**, and gates read Claude's keys:
  - `write_file` and `edit_file` use `path`, not `file_path`. `gates.rs` reads only `file_path`, while edit records fall back to `path`.
  - `edit_file` uses `find`/`replace`, not `old_string`/`new_string`.
  - `read_file` uses `path`.
  - `search` uses `pattern`, `glob` (an array of filters), `mode` (`regex`) and `output_mode` (Grep's `files_with_matches`).
- **The MCP `tool_name` a hook sees** is the model-visible name, `mcp__plugin_<plugin id>_<server id>__<tool>`, with `-` and `.` in the ids turned into `_`. Here that is `mcp__plugin_toolu_probe_540_probe_mcp__probe_echo` for plugin `toolu-probe-540`, server `probe-mcp`, tool `probe_echo`. It starts with `mcp__`, so toolu's `mcp__` matching applies, but the server segment carries the `plugin_<id>_` prefix.
- **Ignore list:** `submit_reminder_decision` and `submit_result` (see Internal tools). Hooks also fire inside `subagent_spawn` children and workflow children with the children's own `session_id`. Those are real tool calls and should be gated normally.
- **Payload quirks:** `transcript_path` is `null`, `PermissionRequest` lacks `tool_use_id`, and `permission_mode` is one of `default`/`bypassPermissions`.

**#544: encoder**

- **Deny:** `PreToolUse` `hookSpecificOutput.permissionDecision: "deny"` (plus `permissionDecisionReason`), or the epic's exit-0 `{"decision":"block"}`. On `PermissionRequest`, `hookSpecificOutput.decision.behavior: "deny"` with `message`.
- **Ask:** `PreToolUse` `hookSpecificOutput.permissionDecision: "ask"`. It prompts under `on-request` only, and it blocks a headless `muse exec` indefinitely. `PermissionRequest` cannot ask.
- **Rewrite:** `hookSpecificOutput.updatedInput` must be the complete input.
- **Context:** `hookSpecificOutput.additionalContext` on `PreToolUse` is accepted (`additional_contexts` in `hook test`). This was not exercised live.
- **PostToolUse block:** the reason becomes the model's entire tool result. Anything the model needs from the original output must be in the reason.
- **Failures:** failed tool calls, including `bash` with a non-zero exit, arrive as `PostToolUseFailure`, so a post-tool gate that must see them has to subscribe to that event too.

**#554: hub hooks and delegation**

- **Delegation tools** on the trusted default surface are `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_wait`, `subagent_read_result` and `subagent_cancel`. They are absent from untrusted sessions.
- **`subagent_spawn` keys** are `command_id`, `context_policy_ref`, `objective`, `output_schema`, `role`, `subagent_type`, `task_name` and `worktree_isolation`, with `command_id`, `role` and `objective` required.
  - It has **no model or reasoning-effort key**.
  - `subagent_type` and `task_name` match the keys `agent_tier` reads, but `agent_tier` only fires for `Agent`, `Task` or `spawn_agent` and compares a `model` that Muse never sends.
- **`workflow` keys** are `name` and `script`.
  - Per-child options live inside the JavaScript string: `host.agent({ input, agentType, model, effort, schema, isolation, label, phase })`, as the model reported in I10.
  - I9b passed `agentType: "general-purpose"`, `model: "muse-spark-1.3-contributor"` and `effort: "low"`, and the child ran on that model.
  - A delegation gate on Muse would have to read `subagent_type` from `subagent_spawn`, and parse `script` to see a workflow child's model or effort.
- **`subagent_wait` and `subagent_send_message`:** the model chose `wait_for: "result_ready"` and `timeout_ms`. A message sent to an already-finished child returns `{"status":"rejected","reason":"terminal"}`.
- **Peer messaging:** `list_peer_sessions` and `send_session_message` are not offered to `muse exec` sessions, so the live acceptance run (#553) should check them in the TUI.
- **MCP:** see the `tool_name` shape under #543.
