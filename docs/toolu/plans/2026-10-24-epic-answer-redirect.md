# Epic answer redirects a worker — Plan

**Date:** 2026-10-24   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-24-epic-answer-redirect-design.md   **Topic:** `toolu epic answer` without an open item

## Evidence and approach
`reply_answer` in `crates/epic-orchestrator/src/attention_ops.rs` gates on `attention::has_live`. Replace the gate with a live-issue check, add `attention::directed` (journal + `Issue.direction`), surface `direction` in `status::document`, then update the skill, recovery reference and generated CLI docs. Reuses `send_prompt`, `Record::new`, existing `attention_ops_test.rs` harness (real engine, real shell prompt script).

## Workstream summary
engine behavior → status field → docs/skill.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "answer delivers to a live issue without an item and journals once", "check": "cargo test -p toolu-epic-orchestrator attention_ops", "ac_refs": ["AC-1", "AC-3", "AC-4"], "paths": ["crates/epic-orchestrator/src/attention_ops.rs", "crates/epic-orchestrator/src/attention.rs", "crates/epic-orchestrator/src/model.rs", "crates/epic-orchestrator/src/tests/attention_ops_test.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs"], "input": "real engine plus shell prompt script; issue 'a' from report execution; unknown key; merged issue; failed prompt script"},
  {"id": "s2", "title": "status issue entry shows the last direction", "check": "cargo test -p toolu-epic-orchestrator status", "ac_refs": ["AC-2"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/status.rs", "crates/epic-orchestrator/src/tests/status_test.rs"], "input": "world with issue a and a recorded direction"},
  {"id": "s3", "title": "skill, recovery reference and CLI docs name answer for redirects", "check": "cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo test -p toolu-cli", "ac_refs": ["AC-5"], "depends_on": ["s2"], "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md", "crates/epic-orchestrator/src/verbs.rs", "docs/cli"], "input": "the real skill Markdown and binary command tree"}
]
```

## Critical files
attention_ops.rs, attention.rs, model.rs, status.rs, verbs.rs, their tests, SKILL.md, recovery.md, docs/cli/epic.md, docs/cli/commands.json.

## Verification
Run the three checks, then `cargo xtask gate` via `toolu epic job --`, and `toolu ledger run <plan> --verify`. Failure cases: unknown key, merged issue, undelivered prompt.
