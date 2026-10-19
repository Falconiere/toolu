# Muse tool inventory and hook-verdict spike — Design

**Date:** 2026-10-19   **Status:** Approved   **Author:** Claude (epic worker, #540)   **Topic:** Observe every tool call a Muse Code 1.4.10 hook can see, and settle the ask, input-rewrite and PostToolUse-block verdicts from live runs, before #543/#544/#545/#554 build on them.

## Problem

Epic #539 maps Muse tool names onto toolu's canonical names (#543), encodes hook decisions for Muse (#544), gates canonical tool calls (#545) and ports the hub hooks (#554). The epic probe saw only four tool names: `bash`, `read_file`, `edit_file` and the internal `submit_reminder_decision`. If the normalize table ships from those four, then write, search, web, delegation and MCP calls stay unenforced or get misclassified. The encoder also needs three verdicts that nobody has observed yet:

- Can a hook make Muse show an ask prompt?
- Can a hook rewrite tool input?
- What does a `PostToolUse` block do to the tool result the model sees?

## Non-Goals

1. No production code: no crate, `hooks.json`, `plugin.json`, `.muse-plugin` manifest or `fixtures/**` change. The issue forbids it, and a fixture with no reading test belongs to the sub-issue that tests it (#543, #544).
2. No marketplace or catalog work; that is #541. The probe never adds a marketplace.
3. No probe script is committed. The probe plugin (Python hooks and a stdio MCP stub) and the extractor stay in the session scratchpad. Their source is quoted in the research doc so they can be reproduced. `final-removal` bans shell scripts, and the probe is throwaway.
4. Not every model, preset or provider combination. The matrix (below) covers:
   - the default surface, untrusted and trusted;
   - the `native-basic` preset;
   - the legacy `--enable-shell-tool` surface;
   - MCP.

   Those are the configurations a toolu user's session can expose.
5. **Intentional scope beyond "write it up in the issue":** the PR commits the same findings as `docs/research/muse-tool-inventory.md`, plus the redacted captures `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`. The issue comment stays the acceptance surface. The committed copy exists because the #543, #544, #545 and #554 workers read the repository, and the worker pipeline delivers a merged PR. Jev chose this form (0.63) over issue-only (0.36) and over committed fixtures (0.01).

## Architecture

**Probe plugin `toolu-probe-540`** (scratchpad, `muse plugins validate` clean, zero diagnostics):

- Hooks: seven logging entries, `["python3","hooks/<event>.py"]`, each with `timeoutMs` 10000. Muse forbids two hooks sharing one source file, so each event has its own entry file over one shared `probe_lib.py`. The events:
  - `PreToolUse`, `PostToolUse`, `PostToolUseFailure` and `PermissionRequest`, for the inventory and verdicts;
  - `SubagentStop`, which marks child and reminder agents;
  - `Notification`, a candidate channel for an approval or ask signal;
  - `SessionStart`, which records the hook environment once per session.
- Logging: each call appends `{configured_event, run_tag, argv, cwd, env (MUSE_/CLAUDE_/PLUGIN_/CODEX_/CURSOR_ only), payload}` to `logs/hooks.jsonl`.
- Answers: each call then answers from `control.json`. The first rule whose `event` and optional `tool` match gives `stdout`, `stderr` and `exit`. No rule means exit 0 with empty output. Changing an experiment therefore needs no reinstall and no re-approval.
- MCP: one stdio server `probe-mcp` (`mcp/server.py`) with one tool, `probe_echo {text}`, which logs its JSON-RPC traffic to `logs/mcp.jsonl`.

**Install, approve, remove** (every command runs with `XDG_DATA_HOME=<scratch>/xdg-data MUSE_UPDATE_INTERVAL_SECONDS=99999999`):

- `muse plugins install <scratch>/probe --json`
- `muse plugins approve toolu-probe-540 --json`
- `muse plugins remove toolu-probe-540 --delete-data --json` at cleanup

**Isolation and side effects (verified before this spec).**

- `XDG_DATA_HOME=<scratch>/xdg-data` puts the plugin lockfile, the plugin cache and the session logs in the scratchpad. That includes each session's `cron.db` and peer history. `--scope project` does not isolate: it was observed writing the global `~/.local/share/muse/plugins/installed.json`.
- Two global writes remain:
  - approval trust, in `~/.config/muse/settings.json` `runtime_capabilities` under keys `plugin:toolu-probe-540:*`. Muse serializes writes with its own `.settings.json.lock`, which keeps them safe against the parallel #541 worker. The keys are unique to this issue.
  - whatever the memory, goal or other tools persist outside the data home. This is unknown until observed.
- So before the first run and after cleanup, the probe snapshots the file lists and mtimes of `~/.config/muse`, `~/.local/share/muse` (excluding `sessions/` and the index database, which other sessions on this machine write), `~/.agents` and `<scratch>/ws`. Every new or changed path the probe caused is named in the doc and removed. A path whose owner is unclear is left alone and reported.
- **Auth source:** the global `~/.config/muse/auth.json`, used in place exactly as any normal Muse session uses it. It is never copied, so a token refresh cannot fork the login, and its contents are never read by the probe.
- `MUSE_UPDATE_INTERVAL_SECONDS=99999999` stops the launcher's hourly self-update, so every run uses `1.4.10-R6427.1`. Each session log's `runtime.session.metadata.build.semver` is checked.

**Ground truth per run.** Each `muse exec --json` run in `<scratch>/ws` (a git repository with one file) gives:

1. the hook log, which is what a hook sees;
2. the session log's `model_request_configured.toolset.active_tools`, the model-visible toolset per agent;
3. `assistant_tool_calls_committed`, the model's call;
4. `tool_result_batch_committed`, the exact tool-result text the model received;
5. every record between that result and the next `assistant_*` record, which catches anything else delivered to the model, such as appended developer or reminder messages.

The session log lives under `<scratch>/xdg-data/muse/sessions/…/session.jsonl`, with child and reminder agents under `subagent/<id>/session.jsonl`. Reminder agents (`skill-reminder`, `goal-reminder`, `verify-reminder`) start on every run without a trigger: the smoke run logged `reminder.agent.*` tasks and `submit_reminder_decision` calls. So every run also samples the internal tools.

**Run matrix.** The model is `meta` / `muse-spark` (the default). Each run's prompt names its family and the exact tool to call. A family the model skips is re-prompted once with stricter wording.

| Run | Flags (besides `exec --json`) | Prompt asks for |
|-----|-------------------------------|-----------------|
| I0 | `--approval-mode never` (untrusted workspace) | nothing; records the untrusted default `active_tools` |
| I1 | `--approval-mode never --trust-workspace` | `write_file` new file, then `write_file` overwrite, `edit_file`, `read_file` |
| I2 | same as I1 | `search` (content and file-name), `read_skill`, `write_todos` |
| I3 | same as I1 | `web_fetch` of `https://example.com`, `web_search` for one query |
| I4 | same as I1 | `add_memory`, `read_memory`, `edit_memory` |
| I5 | same as I1 | `cron_create` (far-future, then `cron_delete`), `cron_list`, `monitor` on a short `bash` command, `bash_input` |
| I6 | same as I1 | `create_goal`, `get_goal`, `update_goal`, `report_progress`, `work_list`, `work_status`, `work_stop`, `snooze_reminder` |
| I7 | same as I1 | `image_generation` of a tiny image |
| I8 | same as I1 | MCP `probe_echo` (the model-visible name is read from `active_tools`) |
| I9 | same as I1 | delegation: `workflow` running one child agent, plus any delegation tool the trusted toolset adds |
| I9b, I10, I11 | same as I1 | added during execution: a `workflow` whose `host.agent` passes `agentType`, `model` and `effort` (I9b); `work_status`/`work_stop` on a real background job plus `subagent_spawn` with every optional parameter, `subagent_status`, `subagent_send_message` and `subagent_cancel` (I10); failing `read_file` and `bash` calls for `PostToolUseFailure` (I11) |
| P1 | `--preset native-basic --approval-mode never --trust-workspace` | `shell`, `subagent_spawn` + `subagent_wait` + `subagent_read_result`, `list_peer_sessions`, `send_session_message` to its own session |
| S1 | `--enable-shell-tool --approval-mode never --trust-workspace` | `shell` naming on the default surface |
| A0 | `--approval-mode untrusted --trust-workspace`, no hook rule | `bash` `touch ask-baseline.txt`; this calibrates Muse's own approval record (see V-ask) |
| A1–A11 | `--approval-mode untrusted --trust-workspace` | the I1–I11 prompts again; a tool is `fired` when a `PermissionRequest` hook line with its `tool_name` appears |
| AP1, AS1 | the P1 and S1 flags with `--approval-mode untrusted` in place of `never` | the P1 and S1 prompts again (`shell`, `subagent_*`, `list_peer_sessions`, `send_session_message`) |

**V-ask** (target `bash` `touch ask-540.txt`, a write that the untrusted mode gates):

- Calibration (A0): with no hook rule, record what Muse's own approval looks like headless:
  - the `PermissionRequest` hook line;
  - the approval records in the session log (the `session_permission_transaction` frame, `approval-review/` entries, or the approval-judge `resolution_source`);
  - any approval event on the `--json` stdout;
  - any `Notification` hook line.

  That set of records is the definition of **prompted** for this host.
- Candidates, each tried on `PreToolUse` and on `PermissionRequest`:
  - `{"decision":"ask","reason":"probe-540 ask"}`
  - `{"permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}`
  - `{"hookSpecificOutput":{"hookEventName":"<event>","permissionDecision":"ask","permissionDecisionReason":"probe-540 ask"}}`
  - `{"decision":"approve"}`
- Each candidate goes first through `muse plugins hook test <id>:<hook> --fixture <payload> --json`, whose classification is recorded. It then runs live with `--trust-workspace` under `--approval-mode on-request` (the default), `untrusted` and `never`.
- Each live outcome is exactly one of:
  - `prompted`: the A0 approval records appear for this call, and the hook test classified the output as an ask, not merely the mode's own prompt. That second condition is checked by also running the candidate under `--approval-mode never`.
  - `blocked`: the tool did not run, and the result says the hook blocked it.
  - `ran`: the hook output was ignored.
  - `failed`: Muse recorded the hook as failed and the tool ran.
  - `not-invoked`: the hook event never fired for this call. For example, `PermissionRequest` does not fire when the mode needs no approval.
- Added during execution, after the hook test showed which values parse: PreToolUse `hookSpecificOutput.permissionDecision: "deny"`, and PermissionRequest `hookSpecificOutput.decision.behavior` `"deny"` and `"allow"`, each run live under `untrusted` only. They are recorded in `verdicts.ask.extra_candidates`.
- If headless exec cannot surface any prompt even for A0, the verdict says so. It then falls back to the `hook test` classification: a candidate classified as an ask counts as an ask surface, and one classified as `failed` or an unsupported field does not.

**V-rewrite** (target `bash` `echo original-540`, under `--approval-mode never --trust-workspace`). The `PreToolUse` candidates:

- `{"updatedInput":{"command":"echo rewritten-540"}}`
- `{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"command":"echo rewritten-540"}}}`
- `{"decision":"approve","updatedInput":{"command":"echo rewritten-540"}}`
- added during execution, after the hook test pointed to it: `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{…}}}`

Each gets the hook-test classification and the live `tool_result_batch_committed` text.

**V-post** (target `edit_file` replacing `alpha` with `alpha-edited` in `notes.txt`, under `--approval-mode never --trust-workspace`). The `PostToolUse` candidates:

- `{"decision":"block","reason":"probe-540 post block"}` with exit 0;
- the stderr text `probe-540 post block` with exit 2.

For each, record:

- the hook-test classification;
- the live tool-result text and the records up to the next assistant message (ground truth 5);
- the model's reply, after the prompt asks it to quote its last tool result verbatim;
- whether `git -C <scratch>/ws diff` shows the edit.

**Deliverables.**

- A findings comment on #540. This is the issue's acceptance surface.
- `docs/research/muse-tool-inventory.md`.
- `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`.

## Interfaces / Schema

Evidence JSON (`docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`). Ids become `<id>`, scratch paths become `<ws>`/`<scratch>`, `$HOME` becomes `~`, and web bodies are truncated to 200 characters.

```json
{
  "muse": { "version": "1.4.10-R6427.1", "captured": "2026-10-19", "model": "muse-spark-…" },
  "toolsets": { "default-untrusted": ["…"], "default-trusted": ["…"], "native-basic": ["…"], "enable-shell-tool": ["…"] },
  "tools": {
    "<tool_name>": {
      "family": "shell|file-read|file-write|file-edit|search|web|skill|memory|cron|monitor|workflow|delegation|peer-message|image|goal|todo|work|reminder|mcp|internal",
      "internal": false,
      "origin": "for internal tools only: the agent and toolset that carry it",
      "configs": ["default-trusted"],
      "exercised": true,
      "not_exercised_reason": null,
      "tool_input_keys": ["…"],
      "post_event": "PostToolUse|PostToolUseFailure",
      "tool_response_shape": "string|object with keys …",
      "permission_request": "fired|not fired|not exercised",
      "pre_example": {},
      "post_example": {}
    }
  },
  "permission_request_payload_keys": ["…"],
  "side_effects": [{ "path": "~/…", "cause": "<tool>", "removed": true, "reason": "set only when removed is false" }],
  "cleanup": { "sandbox_list": "no plugins", "global_list": "<the global muse plugins list output at cleanup time>" },
  "verdicts": {
    "ask": {
      "calibration": { "approval_records": ["…"], "notification": "…" },
      "candidates": [{ "event": "PreToolUse|PermissionRequest", "output": "…", "hook_test": "…", "live": { "on-request": "prompted|blocked|ran|failed|not-invoked", "untrusted": "…", "never": "…" }, "evidence": "…" }],
      "conclusion": "supported shape … | no ask surface"
    },
    "updated_input": {
      "candidates": [{ "output": "…", "hook_test": "…", "tool_result": "…", "ran": "original-540|rewritten-540|none" }],
      "conclusion": "…"
    },
    "post_tool_use_block": {
      "candidates": [{ "output": "…", "exit": 0, "hook_test": "…", "tool_result": "…", "follow_up_records": ["…"], "model_quote": "…", "persisted": true }],
      "conclusion": "…"
    }
  }
}
```

The research doc's sections, whose headings the plan checks:

- `## Method`: probe source in `text` fences, so `check-markdown-cli` stays clean without adding `muse` to `external`, and no inline span starting with `toolu` plus a lowercase word unless it is a real `toolu` command;
- `## Toolsets`;
- `## Tool inventory`: `tool_name`, family, `tool_input` keys, response shape, `PermissionRequest`, proposed canonical name;
- `## Examples`: one redacted payload per family;
- `## Internal tools`;
- `## Ask verdict`;
- `## Input-rewrite verdict`;
- `## PostToolUse-block verdict`;
- `## Side effects and cleanup`;
- `## Notes for #543, #544 and #554`.

## Failure modes and edge cases

- **The model does not call the requested tool.** Re-prompt once and name the exact tool. If it still declines, the tool gets `exercised: false` with a `not_exercised_reason`, and its toolset membership is still recorded. It is never silently dropped.
- **A tool is listed but fails** (entitlement, sandbox network, missing setup). Its failure result and `PostToolUseFailure` payload are recorded as observed behaviour, not retried around.
- **A hook does not fire.** The extractor cross-checks each run's `assistant_tool_calls_committed` calls against hook-log lines for the same session. A call with no `PreToolUse` line is reported as "hook not invoked for <tool>", which is an enforcement finding in the doc.
- **Hook timeout or crash.** The log records nothing for that call, and the cross-check above surfaces it.
- **Parallel interference.** `run_tag` and `session_id` scope every log line, and only this run's session ids are used. The #541 worker's plugins live in the global store, which these sandboxed sessions never load.
- **Self-update mid-run.** A session whose `build.semver` is not `1.4.10` is discarded and re-run.
- **Persistent side effects.** These come from `cron_create`, memory, goals, `send_session_message` and `image_generation`. Each is found by the before/after snapshot, then named, and then removed through the tool itself where possible (`cron_delete`, `edit_memory`) or deleted. A path whose owner is unclear is reported and left alone. External cost is bounded to one `web_search`, one `web_fetch` and one tiny `image_generation`.
- **Secrets.** Only prefixed environment variables are logged. Before committing, the evidence and doc pass `! grep -nE 'Bearer [A-Za-z0-9]|sk-[A-Za-z0-9]{16}|access_token|refresh_token|"token"|/root/' <doc> <evidence>`. A hit blocks the commit.
- **Cleanup interrupted.** Removal runs even after a failed run. If `runtime_capabilities` keys survive removal, `muse plugins reject toolu-probe-540` is tried, then removal again. A key that still survives is reported in the doc; the user's settings file is never hand-edited.

## Acceptance criteria

- **AC-1:** Given the probe on Muse 1.4.10, the research doc and evidence list every tool name that is in any recorded main-agent `active_tools` or in any `PreToolUse` hook line of runs I0–I11, I9b, P1 and S1. Each entry carries:
  - its families and configurations;
  - for an exercised tool, its `tool_input` keys, its post event with the `tool_response` shape, and a redacted `pre_example`;
  - for a listed tool that was not exercised, `exercised: false` and the reason.

  Every family in {shell, file-read, file-write, file-edit, search, web, skill, memory, cron, delegation, mcp} has at least one exercised tool with an example.
- **AC-2:** Given runs A1–A11, AP1 and AS1 under `--approval-mode untrusted`, every exercised non-internal tool has `permission_request` ∈ {fired, not fired, not exercised}, decided by the presence of a `PermissionRequest` hook line with that `tool_name`. The `PermissionRequest` payload keys are listed, or the list is empty because no tool fired.
- **AC-3:** The ignore list names every host-internal tool observed, at least `submit_reminder_decision`. Each entry is absent from every main-agent `active_tools`, and its `origin` cites the reminder or child agent and toolset (`mode: named, source: runtime`) that carries it.
- **AC-4:** The ask verdict records the A0 calibration records, and for each candidate × {PreToolUse, PermissionRequest} the hook-test classification and the live outcome under on-request, untrusted and never. Its conclusion is either a supported ask shape or "no ask surface".
- **AC-5:** The input-rewrite verdict quotes the live tool-result text for `bash` `echo original-540` under each candidate, and concludes whether `rewritten-540` ever ran.
- **AC-6:** The PostToolUse-block verdict quotes what the model received for the `edit_file` call under each candidate: the tool result, the follow-up records and the model's verbatim quote. It also states whether the edit persisted on disk.
- **AC-7:** The `## Notes for #543, #544 and #554` section records:
  - the input keys of every delegation tool exercised (`workflow`, `subagent_spawn` and any trusted-only delegation tool), naming its model, agent type or role, effort and prompt keys or their absence;
  - the exact MCP `tool_name` a hook sees for `probe_echo`.
- **AC-8:** After cleanup:
  - neither the global nor the sandbox `muse plugins list` names `toolu-probe-540`;
  - the global list at cleanup time is recorded verbatim (`cleanup.global_list`) and names no probe. If it is not empty, the doc and comment state that its entries belong to the parallel #541 worker, since the issue expects `muse plugins list` to be empty;
  - `muse plugins marketplace list` names no `toolu-probe-540` marketplace (the parallel #541 worker registers its own `probe-*` marketplaces globally, so only this id is asserted);
  - `~/.config/muse/settings.json` has no `plugin:toolu-probe-540` key.
- **AC-9:** #540 carries the findings as one comment titled "Muse tool inventory". When the body would exceed 60,000 characters (GitHub caps a comment at 65,536), it is split into numbered parts "Muse tool inventory (n/m)". Together the parts contain:
  - every evidence tool with each of its `tool_input` keys;
  - one redacted example per exercised family, headed `Example — <family>`;
  - the ignore list (`Internal tools`);
  - the three verdict headings and the notes for #554.

  The docs-only diff passes `cargo xtask check-markdown-cli`, `cargo xtask gate --only context-budget --only ci-paths` and the full `cargo xtask gate`.
- **AC-10:** Every persistent path the probe created outside the scratchpad appears in `side_effects` with `removed: true`. A path not removed carries the reason, and the doc names it.

## Acceptance evidence

The extractor (`extract.py`, scratchpad) turns the live logs into the evidence JSON. Its cross-check runs once, during execution, and the doc records it. The ledger checks read only committed or live state, so `toolu ledger run --verify` can repeat them.

| AC | Real input | Expected observable result | Boundary / failure case | Runnable check |
|----|-----------|---------------------------|-------------------------|----------------|
| AC-1 | hook log and session logs of I0–I11, I9b, P1, S1 | every `active_tools` ∪ hook-log name is an evidence key; each exercised tool has keys, a post event and an example; every required family is exercised | listed but unexercised → `exercised: false` with reason; a required family missing fails | `jq -e` over the evidence (the plan's S1 expression); the cross-check output is pasted in the doc's Method section |
| AC-2 | hook log of A1–A11, AP1, AS1 | `permission_request` set for every exercised tool; payload keys non-empty whenever a tool fired | a tool not run under untrusted → `not exercised`; no tool fired → empty key list allowed | `jq -e` (plan S3) |
| AC-3 | reminder-agent session logs | `internal` tools have an `origin` and appear in no `toolsets` list | an internal tool listed in a main toolset fails | `jq -e` (plan S2) |
| AC-4 | A0 records, hook-test JSON, live V-ask sessions | calibration non-empty; every candidate has a hook test and three live outcomes; conclusion set | headless exec shows no prompt even for A0 → stated, with the hook-test fallback | `jq -e` (plan S4) |
| AC-5 | live V-rewrite sessions | `ran` ∈ {original-540, rewritten-540, none} per candidate, plus the quoted result | an empty or failed result is recorded verbatim (`none`) | `jq -e` (plan S4) |
| AC-6 | live V-post sessions and `git diff` | tool result, follow-up records, model quote and `persisted` per candidate | exit-2 fail-open recorded as such | `jq -e` (plan S4) |
| AC-7 | hook lines of I8, I9 and P1 | delegation and mcp entries with `tool_input_keys`; notes section present | delegation unavailable → the host message recorded | `jq -e` (plan S1) plus the doc heading grep (plan S5) |
| AC-8 | live `muse` state plus `cleanup` in the evidence | no `toolu-probe-540` in the global list, the marketplace list or the settings; the sandbox list recorded as `no plugins` right after removal | the parallel worker's plugins may exist globally; only the probe's absence is asserted | plan S6 |
| AC-9 | GitHub issue and repository | the findings comment (or its numbered parts) names every evidence tool and each of its `tool_input` keys, has an `Example — <family>` per exercised family, the ignore list and the three verdict headings; gates exit 0 | body over 60,000 characters → numbered parts, checked together | plan S7 (`gh api …/comments` over all parts, looped over the evidence tools, keys and families) and S8 |
| AC-10 | before/after snapshots | each `side_effects` entry is `removed: true` and its path no longer exists, or carries a `reason` | owner unclear → reason recorded | plan S6 (`jq` over `side_effects` plus `test ! -e` on each removed path) |

## Documentation impact

- New `docs/research/muse-tool-inventory.md` (research report) and `docs/toolu/evidence/2026-10-19-muse-tool-inventory.json`.
- No user-facing docs change: Muse support is not shipped. `docs/runtime.md`, `docs/install.md` and the CLI references are updated by #551 once the host lands.

## Open Questions

None blocking.

- Decided: the deliverable is the issue comment plus a research doc plus an evidence JSON, not fixtures (Non-Goal 5).
- Decided: isolation uses `XDG_DATA_HOME`, not `--scope project`. Reason: project scope was observed writing the global lockfile.
- Decided: "prompted" is calibrated per host by A0, not assumed from Claude's or Codex's prompt shape.
