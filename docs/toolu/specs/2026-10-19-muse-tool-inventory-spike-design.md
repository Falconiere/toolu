# Muse tool inventory and hook-verdict spike — Design

**Date:** 2026-10-19   **Status:** Draft   **Author:** Claude (epic worker, #540)   **Topic:** Observe every tool call a Muse Code 1.4.10 hook can see, and settle the ask, input-rewrite and PostToolUse-block verdicts from live runs, before #543/#544/#545/#554 build on them.

## Problem

Epic #539 maps Muse tool names onto toolu's canonical names (#543), encodes hook decisions for Muse (#544), gates canonical tool calls (#545) and ports the hub hooks (#554). The epic probe saw only four tool names: `bash`, `read_file`, `edit_file` and the internal `submit_reminder_decision`. If the normalize table ships from those four, then write, search, web, delegation and MCP calls stay unenforced or get misclassified. The encoder also needs three verdicts that nobody has observed yet:

- Can a hook make Muse show an ask prompt?
- Can a hook rewrite tool input?
- What does a `PostToolUse` block do to the tool result the model sees?

## Non-Goals

1. No production code: no crate, `hooks.json`, `plugin.json`, `.muse-plugin` manifest or `fixtures/**` change. The issue forbids it, and a fixture with no reading test belongs to the sub-issue that tests it (#543, #544).
2. No marketplace or catalog work. That is #541.
3. No probe script is committed. The probe plugin (Python hooks and a stdio MCP stub) lives in the session scratchpad, and its source is quoted in the research doc so it can be reproduced. `final-removal` bans shell scripts, and this probe is throwaway.
4. Not every model, preset or provider combination. The matrix covers the default surface, the `native-basic` preset, the legacy `--enable-shell-tool` surface, a trusted workspace (delegation), and MCP. Those are the configurations a toolu user's session can expose.

## Architecture

**Probe plugin `toolu-probe-540`** (scratchpad, `muse plugins validate` clean, zero diagnostics):

- seven logging hooks, `["python3","hooks/<event>.py"]`, for `PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`, `SubagentStop`, `Notification` and `SessionStart`. Muse forbids two hooks sharing one source file, so each event has its own entry file over one shared `probe_lib.py`;
- each call appends `{configured_event, run_tag, argv, cwd, env (MUSE_/CLAUDE_/PLUGIN_/CODEX_/CURSOR_ only), payload}` to `logs/hooks.jsonl`. It then answers from `control.json`: the first rule whose `event` and optional `tool` match gives `stdout`, `stderr` and `exit`, and no rule means exit 0 with empty output. Changing a verdict experiment therefore needs no reinstall and no re-approval;
- one stdio MCP server `probe-mcp` (`mcp/server.py`) with one tool, `probe_echo {text}`, which logs its JSON-RPC traffic to `logs/mcp.jsonl`.

**Isolation (verified before this spec).** The #541 worker probes Muse in parallel on the same machine, so the probe must not touch their plugin store:

- `XDG_DATA_HOME=<scratch>/xdg-data` puts the plugin lockfile, cache and session logs in the scratchpad. `--scope project` does not isolate: it still wrote the global `~/.local/share/muse/plugins/installed.json`. With the variable set, the global `muse plugins list` keeps printing `no plugins`.
- Approval trust is the one global write. It goes to `~/.config/muse/settings.json` `runtime_capabilities`, under keys `plugin:toolu-probe-540:*`, and must be gone after removal.
- `MUSE_UPDATE_INTERVAL_SECONDS=99999999` stops the launcher's hourly self-update, so every run uses `1.4.10-R6427.1`. Each session log's `runtime.session.metadata.build.semver` is checked.
- Provider credentials are not copied, so a token refresh cannot fork the user's login.

**Ground truth per run.** Each `muse exec --json` run in `<scratch>/ws` (a git repository with one file) gives four views:

1. the hook log, which is what a hook sees;
2. the session log's `model_request_configured.toolset.active_tools`, the model-visible toolset;
3. `assistant_tool_calls_committed`, the model's call;
4. `tool_result_batch_committed`, the exact text the model received.

The session log lives under `<scratch>/xdg-data/muse/sessions/…/session.jsonl`.

**Run matrix.** Each prompt names one tool family and the exact tool to call. A family the model skips is re-prompted once with stricter wording.

| Run | Flags | Exercises |
|-----|-------|-----------|
| I1–In | default surface, `--approval-mode never`, `--trust-workspace` | `write_file` (new + overwrite), `edit_file`, `read_file`, `search`, `web_fetch`, `web_search`, `read_skill`, `add_memory`/`read_memory`/`edit_memory`, `cron_create`/`cron_list`/`cron_delete`, `monitor`, `workflow`, `image_generation`, `write_todos`, goal and work tools, `bash`/`bash_input`, `probe_echo` (MCP), subagent delegation |
| P1 | `--preset native-basic` | `shell`, `subagent_*`, `list_peer_sessions`, `send_session_message` |
| S1 | `--enable-shell-tool` | the legacy `shell` name on the default surface |
| A1–An | the same prompts with `--approval-mode untrusted` | which tools fire `PermissionRequest` |
| V-ask | `control.json` rules on `PreToolUse` and `PermissionRequest` | `muse plugins hook test` classification, then live outcome, of `{"decision":"ask"}`, `{"permissionDecision":"ask"}`, `{"hookSpecificOutput":{"hookEventName":…,"permissionDecision":"ask"}}` and `{"decision":"approve"}` |
| V-rewrite | `PreToolUse` on `bash` | `{"updatedInput":{…}}`, `hookSpecificOutput.updatedInput` and `{"decision":"approve","updatedInput":…}`, which rewrite `echo original-540` into `echo rewritten-540`; the tool result shows which ran |
| V-post | `PostToolUse` on `edit_file` | `{"decision":"block","reason":"probe-540 post block"}` with exit 0, and stderr text with exit 2; the tool result text and the file on disk show the effect |

**Deliverables.**

- A findings comment on #540, which is the issue's acceptance surface.
- `docs/research/muse-tool-inventory.md`, the durable copy the later workers read. Jev chose this over issue-only (0.63 vs 0.36) and over committing fixtures (0.01).
- `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`, the redacted machine-readable captures.

## Interfaces / Schema

Evidence JSON (`docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`):

```json
{
  "muse": { "version": "1.4.10-R6427.1", "captured": "2026-10-19" },
  "redaction": "ids → <id>, scratch paths → <ws>/<scratch>, $HOME → ~, web bodies truncated",
  "toolsets": { "default": ["…"], "default-trusted": ["…"], "native-basic": ["…"], "enable-shell-tool": ["…"] },
  "tools": {
    "<tool_name>": {
      "family": "shell|file-read|file-write|file-edit|search|web|skill|memory|cron|monitor|workflow|delegation|peer-message|image|goal|todo|work|mcp|internal",
      "internal": false,
      "tool_input_keys": ["…"],
      "post_event": "PostToolUse|PostToolUseFailure",
      "tool_response_shape": "string|object keys …",
      "permission_request": "fired|not fired|not exercised",
      "pre_example": {},
      "post_example": {}
    }
  },
  "verdicts": { "ask": {}, "updated_input": {}, "post_tool_use_block": {} }
}
```

The research doc has these sections:

- method and reproduction (probe source in `text` fences, so that `check-markdown-cli` stays clean without adding `muse` to `external`);
- toolsets per configuration;
- tool table (`tool_name`, family, `tool_input` keys, response shape, `PermissionRequest`, proposed canonical name);
- one redacted example payload per family;
- the internal-tool ignore list with its evidence;
- the ask, input-rewrite and PostToolUse-block verdicts with probe outputs;
- notes for #543, #544 and #554 (delegation keys, MCP `tool_name` shape).

## Failure modes and edge cases

- **The model does not call the requested tool.** Re-prompt once and name the exact tool. If it still declines, the tool is listed as `not exercised` with the reason, and its toolset membership is still recorded. It is never silently dropped.
- **A tool is listed but fails** (entitlement, sandbox network, missing setup). The failure result and `PostToolUseFailure` payload are recorded as observed behaviour, not retried around.
- **A hook does not fire.** The run's `assistant_tool_calls_committed` calls are cross-checked against the hook-log entries for the same session. A call with no hook entry is reported as "hook not invoked for <tool>", which is itself an enforcement finding.
- **Hook timeout or crash.** The log records nothing for that call, and the cross-check above surfaces it. `timeoutMs` is 10000, and the hook only appends a line.
- **Parallel interference.** `run_tag` and `session_id` scope every log line, and only lines from this run's session ids are used. A foreign `toolu-probe-540` key in global settings is impossible, because the id is unique to this issue.
- **Self-update mid-run.** Pinned by `MUSE_UPDATE_INTERVAL_SECONDS`. A session whose `build.semver` is not `1.4.10` is discarded and re-run.
- **Secrets.** Only prefixed environment variables are logged. Before committing, the evidence and doc are scanned for token-shaped values (`meta`, `Bearer`, `sk-`, long base64) and for `auth.json` content. The commit is blocked on any hit.
- **Cleanup interrupted.** Removal runs even after a failed run (`muse plugins remove toolu-probe-540 --delete-data` with the sandbox `XDG_DATA_HOME`). If `runtime_capabilities` keys survive, `muse plugins reject` is tried before the final check. Hand-editing the user's settings is the last resort and is reported.

## Acceptance criteria

- **AC-1:** Given the probe on Muse 1.4.10, the research doc and evidence list every non-internal `tool_name` observed by `PreToolUse` in runs I*/P1/S1. Each entry gives its `tool_input` keys, its post event (`PostToolUse` or `PostToolUseFailure`) with its `tool_response` shape, and one redacted example payload for each family exercised. The doc also lists `active_tools` for the default, trusted, `native-basic` and `--enable-shell-tool` configurations, and marks tools that were listed but not exercised.
- **AC-2:** Given `--approval-mode untrusted` runs over the same families, the doc records, per exercised tool, whether a `PermissionRequest` hook entry with that `tool_name` appeared, and lists the `PermissionRequest` payload keys.
- **AC-3:** The ignore list names every host-internal tool observed (at least `submit_reminder_decision`). Each entry has evidence: the tool is absent from the main run's `active_tools`, and it is invoked inside a reminder or child agent (session log task kind `reminder.agent.*`, or a paired `SubagentStop`).
- **AC-4:** The ask verdict records, for each candidate output on `PreToolUse` and `PermissionRequest`, the `muse plugins hook test` result and the live-run outcome (prompted, blocked, ignored or failed). It concludes either with the supported ask shape or with "no ask surface".
- **AC-5:** The input-rewrite verdict quotes the live `tool_result_batch_committed` text for `bash` `echo original-540` under each candidate rewrite output, and concludes whether `rewritten-540` ever ran.
- **AC-6:** The PostToolUse-block verdict quotes what the model received for an `edit_file` call whose `PostToolUse` hook returned `{"decision":"block","reason":"probe-540 post block"}` (exit 0), and for the exit-2 + stderr variant. It also states whether the edit persisted on disk.
- **AC-7:** A section for #554 (and #543) records the delegation tools' input keys (model, subagent type or role, effort, prompt and any others), and the exact MCP `tool_name` that a hook sees for `probe-mcp`'s `probe_echo`.
- **AC-8:** After the probe, both the global and the sandbox `muse plugins list` print `no plugins`, and `~/.config/muse/settings.json` has no `plugin:toolu-probe-540` key.
- **AC-9:** #540 carries one findings comment covering AC-1–AC-7, and the docs-only diff passes the repository's docs checks (`cargo xtask check-markdown-cli`, `cargo xtask gate --only context-budget --only ci-paths`) and the full `cargo xtask gate`.

## Acceptance evidence

| AC | Real input | Expected observable result | Boundary / failure case | Runnable check |
|----|-----------|---------------------------|-------------------------|----------------|
| AC-1 | `logs/hooks.jsonl` and the session logs of runs I*/P1/S1 | every logged `PreToolUse` `tool_name` with `internal=false` is a key of `evidence.tools`, with a non-empty `tool_input_keys` | a listed-but-unexercised tool appears as `not exercised`, not missing | `python3 -I <scratch>/check_evidence.py coverage` exits 0 (it diffs hook-log tool names against evidence keys) |
| AC-2 | hook log of runs A* | each exercised tool has `permission_request` ∈ {fired, not fired}, matching a `PermissionRequest` line or its absence | a tool that never ran under `untrusted` is `not exercised` | `check_evidence.py permission` exits 0 |
| AC-3 | session logs (`reminder.agent.*` tasks, `active_tools`) | every `internal=true` tool is absent from the main `active_tools` and has a cited reminder or child-agent origin | a tool both internal and listed would fail the check | `check_evidence.py internal` exits 0 |
| AC-4 | `muse plugins hook test` JSON plus live run session log | each candidate has a hook-test outcome and a live outcome; the verdict string is present | an unrecognized output (`failed`) is recorded as such | `check_evidence.py verdicts` exits 0, and the doc section quotes the outputs |
| AC-5 | live run session log, `tool_result_batch_committed` | the quoted result text contains `original-540` or `rewritten-540` | an empty or failed result is recorded verbatim | `check_evidence.py verdicts` |
| AC-6 | live run session log and `git -C <ws> diff` | the quoted tool-result text, plus a file-persisted yes/no | the exit-2 variant is recorded even if it fails open | `check_evidence.py verdicts` |
| AC-7 | hook log of delegation and MCP runs | `tool_input_keys` of each delegation tool and the exact MCP `tool_name` string | delegation unavailable → recorded with the host's message | `check_evidence.py coverage` (the delegation and MCP families must be present) |
| AC-8 | live `muse` state | `no plugins` twice, and no settings key | — | `muse plugins list`, `XDG_DATA_HOME=<scratch>/xdg-data muse plugins list`, `jq 'any(.runtime_capabilities // {} \| keys[]; startswith("plugin:toolu-probe-540"))' ~/.config/muse/settings.json` → `false` |
| AC-9 | GitHub issue and repository | the comment exists; the gates exit 0 | — | `gh api repos/Falconiere/toolu/issues/540/comments --jq '.[].body' \| grep -c 'Muse tool inventory'` ≥ 1; `cargo xtask check-markdown-cli`; `cargo xtask gate` |

`check_evidence.py` is a scratchpad script. It is real-data only: it reads the captured logs and the committed evidence JSON. Its source is quoted in the plan evidence.

## Documentation impact

- New `docs/research/muse-tool-inventory.md` (research report) and `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`.
- No user-facing docs change: Muse support is not shipped. `docs/runtime.md`, `docs/install.md` and the CLI references are updated by #551 once the host lands.

## Open Questions

None blocking.

- Decided: the deliverable is the issue comment plus a research doc plus an evidence JSON, not fixtures. Reason: the issue forbids production changes, and later workers read the repository (Jev 0.63).
- Decided: isolation uses `XDG_DATA_HOME`, not `--scope project`. Reason: project scope was observed writing the global lockfile.
