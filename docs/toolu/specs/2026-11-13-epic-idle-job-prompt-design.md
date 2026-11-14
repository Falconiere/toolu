# Prompt idle epic workers for finished jobs — Design

**Date:** 2026-11-13   **Status:** Approved   **Author:** Codex   **Topic:** Deliver detached job results to idle workers

## Problem

A detached `toolu epic job` can finish after its worker stops calling tools. The result is saved, but an idle worker needs the engine to put the result in its pane without polling.

## Non-Goals

1. Change detached job execution, result storage, or the hook delivery path.
2. Prompt a worker while its agent status is working, blocked, done, or unknown.

## Architecture

Reuse `job_results::claim_finished(root, worktree)` for one-time delivery and `babysit::prompt(world, key, "job", text, None)` for the journal and prompt steps. A shared engine helper checks that the issue has an idle agent and a worktree, claims finished results, and prompts once per claimed result. The socket's `job-done` event and an agent's transition to idle in either a pushed status event or snapshot invoke it. `schedule::note_event` recognizes `job-done` so the existing event dispatch remains the route from `announce`.

## Interfaces / Schema

The existing socket request is `{ "op": "event", "key": "<issue key>", "event": "job-done", "protocol": <greeting protocol> }`. The result path and schema remain in `toolu_engine::resources::job_results`. The prompt name is `job`; its text starts with `job_results::line(record)` and includes `record.output` after a nonzero exit code.

## Failure modes and edge cases

- Missing issue, worktree, or live idle agent: leave results unclaimed and queue nothing, including after an issue enters cleanup or merges.
- Duplicate event or repeated idle status: the claim returns no delivered records; queue nothing.
- Result finishes while the agent works: leave it unclaimed until the agent becomes idle.
- Two finished results: claim and prompt each once, in the result store's order.
- A result read or claim error: a socket `job-done` request reports the error. A Herdr idle update journals `job-prompt-failed` and continues handling the other workers and status effects; do not invent a completion prompt.

## Acceptance criteria

- **AC-1:** Given one finished result for an idle issue worktree, a `job-done` event journals one `prompt job` row and queues one prompt containing the result line; a second event adds neither.
- **AC-2:** Given a finished result while the agent is working, `job-done` queues nothing; changing that agent to idle queues one `job` prompt.
- **AC-3:** An idle transition observed through a snapshot also delivers a finished result once; non-idle or repeated status updates do not duplicate it.

## Acceptance evidence

- **AC-1:** A real schema-1 result created and completed under a temporary resource root, then socket event dispatch; inspect journal and queued prompt. Check: `cargo test -p toolu-epic-orchestrator --lib dispatch::tests::job_done`.
- **AC-2:** The same real result plus a pushed Herdr status event from working to idle; inspect pending record and prompt. Check: `cargo test -p toolu-epic-orchestrator --lib source_apply::tests::finished_job`.
- **AC-3:** A real result plus a Herdr snapshot moving working to idle and a repeated snapshot; inspect the journal and prompt count. Check: `cargo test -p toolu-epic-orchestrator --lib source_apply::tests::finished_job`.

## Documentation impact

Update the epic orchestrator documentation for the new `job-done` event and idle delivery behavior. No CLI flag or JSON schema changes.

## Open Questions

None.
