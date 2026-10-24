# Epic answer redirects a worker without an open item — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** `toolu epic answer` works on any live worker

## Problem
`toolu epic answer <key> <text>` refuses with `no open attention for <key>` unless the engine raised an attention item. When the orchestrator re-scopes an issue whose worker idles in babysit, the only route is `herdr agent prompt`, which the engine never journals and `toolu epic status` never shows. The epic-orchestrator skill still points at `toolu epic answer` for this.

## Non-Goals
1. No new verb; `answer` keeps its name, arguments and `--by`.
2. No change to answers on an open item: they still mark the item answered.
3. No durable per-issue direction store beyond the journal.

## Architecture
`reply_answer` (crates/epic-orchestrator/src/attention_ops.rs) drops the `has_live` precondition and requires instead that the key names a live issue: present in the world with a stage not in `cleaning`, `cleanup-incomplete`, `merged`, `abandoned`. It sends the prompt through the existing `send_prompt`. With a live item, `attention::answered` runs as before. Without one, a new `attention::directed` journals one `prompt`/`answer` record (empty token, note = text) and sets `Issue.direction` to `{text, by, at_ms}`. `status::document` shows that value on the issue entry, so `toolu epic status <epic>` lists it. Decision: a live issue, not a bound pane, is the test, because `send_prompt` already resolves the agent name and reports delivery failure.

## Interfaces / Schema
- Socket op `answer` unchanged: `{"op":"answer","key","text","by"}`.
- Refusal for a key with no live issue: `no live agent for <key>`.
- Status issue entry gains `"direction": null | {"text","by","atMs"}`.
- Journal: `prompt answer <key>` with note = message text, written once per delivered message.

## Failure modes and edge cases
- Empty key or text: `answer needs a key and a text` (unchanged).
- Unknown key, or issue merged/abandoned/cleaning: `no live agent for <key>`; nothing is prompted or journaled.
- Prompt not delivered: `worker prompt not delivered: ...`; nothing journaled, `direction` unchanged.
- Journal write fails after delivery: the error propagates as today.
- Open item present: behaviour identical to today (item answered, one `prompt answer`, one `judgment answered`).

## Acceptance criteria
- **AC-1:** Given a running issue with no attention item, `answer` delivers the text to the prompt script once and journals exactly one `prompt answer` row naming the key and the text.
- **AC-2:** After AC-1, the status document's issue entry carries `direction.text` equal to the message and `direction.by`.
- **AC-3:** For a key with no issue, or a merged issue, `answer` returns `no live agent for <key>`, calls no prompt, journals nothing.
- **AC-4:** Answering with an open item still yields the existing `prompt answer` + `judgment answered` rows and `state=answered`.
- **AC-5:** `skills/epic-orchestrator/SKILL.md` and `references/recovery.md` name `toolu epic answer` for redirecting a worker and no step says `herdr agent prompt`; `docs/cli/epic.md` and the CLI snapshot match the binary.

## Acceptance evidence
- AC-1/AC-2: `tests/attention_ops_test.rs` with the real engine and the real shell `prompt` script, issue created by a `report execution`; reads the prompts file and journal rows; failure case: script outcome `failed` leaves no row.
- AC-3: same harness, unknown key and an issue whose stage is `merged`.
- AC-4: the existing answer test, kept green.
- AC-5: `cargo xtask docs-cli`, `cargo xtask check-markdown-cli`, and `cargo test -p toolu-cli` skill text tests.

## Documentation impact
`about` text of `answer` in `crates/epic-orchestrator/src/verbs.rs`, regenerated `docs/cli/epic.md` and `docs/cli/commands.json`, `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`, `references/recovery.md`, and docs/epic-orchestrator if it describes answer.

## Open Questions
None blocking. The `--by` default and the journal row format reuse existing behaviour (owner: implementer).
