# A worker the launch never confirmed — Plan

**Date:** 2026-10-24   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-24-cursor-uncertain-lease-design.md   **Topic:** Falconiere/toolu#625 — loadable adopted record, running lease on adoption, session repair on reprompt

## Evidence and approach
Inspected: `launch/record.rs` `check_identity` (the refusal), `logic.rs` `adopt`, and
`disk.rs` `write_issue`/`write_issue_fields`. Adoption writes the record stage only.
`core/engine/src/resources/leases.rs` `admit_worktree_job` admits only
`starting`/`running`. `launch/agent.rs` `require_recorded_session` / `keep_or_stop`,
`launch/attempt.rs` `start`, `launch/deliver.rs` and `launch/session.rs`
`capture_session` (Cursor chat store `<config>/chats/<md5(cwd)>/<id>/meta.json`, checked
against a real `~/.cursor/chats` entry) complete the path. `rebase.rs` `set_stage` is the
existing engine pattern for patching a lease stage. Test rig: `tests/launch_rig_test.rs`
(`Rig`, the herdr socket fixture with real agent processes, `script()` errors). The
precedent for a scripted herdr failure is `launch/tests/attempt_test.rs`. #622 and #628
already fix the busy-pane race and the rewrite of an abandoned record.

Approach (spec, Jev p=0.92): relax validation for the adopted shape only; mirror
`running` onto the agent lease when the engine writes a `running` issue (best effort,
under a zero-wait launch lock, only from `starting`/`uncertain` without `pendingHost`);
capture a missing session for a kept live agent from the host's own store.

## Workstream summary
validation → lease mirror → session repair → end-to-end Cursor stall flow → dry-run
recovery flags → recovery doc → full gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "an adopted running record with an unconfirmed prompt may lack its session; acknowledged ones still may not", "check": "cargo test -p toolu-epic-orchestrator launch::record::tests", "ac_refs": ["AC-1"], "paths": ["crates/epic-orchestrator/src/launch/record.rs", "crates/epic-orchestrator/src/launch/tests/record_test.rs"], "input": "real record JSON files: the #625 shape (running, submitting, session_id null, lease_token); boundaries: the same shape at awaiting_merge, running+acknowledged+null, awaiting_merge+acknowledged+null, running without prompt_state+null, acknowledged managed running without session", "model": "inherit"},
  {"id": "s2", "title": "the engine's running write gives an uncertain or starting agent lease stage running, and nothing else", "check": "cargo test -p toolu-epic-orchestrator launch::adoption::tests && cargo test -p toolu-epic-orchestrator disk::tests", "ac_refs": ["AC-2", "AC-3"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/launch/adoption.rs", "crates/epic-orchestrator/src/launch/tests/adoption_test.rs", "crates/epic-orchestrator/src/launch.rs", "crates/epic-orchestrator/src/disk.rs", "crates/epic-orchestrator/src/tests/disk_test.rs"], "input": "real TOOLU resource root with an agent lease (acquire_lease + patch_lease stage uncertain) and a Job lease request for its worktree; boundaries: launch lock held, pendingHost via prepare_agent_migration, stage replacing, abandoned record, record without lease; resource bytes compared", "model": "inherit"},
  {"id": "s3", "title": "a kept live agent without a recorded session captures the one fresh session its host wrote for the worktree", "check": "cargo test -p toolu-epic-orchestrator launch::agent::tests && cargo test -p toolu-epic-orchestrator launch::", "ac_refs": ["AC-5"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/launch/agent.rs", "crates/epic-orchestrator/src/launch/tests/agent_test.rs", "crates/epic-orchestrator/src/launch/attempt.rs", "crates/epic-orchestrator/src/launch/deliver.rs", "crates/epic-orchestrator/src/launch/session.rs", "crates/epic-orchestrator/src/tests/launch_rig_test.rs", "crates/epic-orchestrator/src/launch.rs"], "input": "Rig herdr fixture with a live cursor-kind agent in a real worktree and Cursor chat dirs in Cursor's on-disk format under HOME/.cursor/chats; boundaries: no chat, two fresh chats, one chat older than session_started_after, no session_started_after", "model": "inherit"},
  {"id": "s4", "title": "a stalled Cursor prompt is admitted for jobs once the engine adopts it and is repaired by --reprompt", "check": "cargo test -p toolu-epic-orchestrator launch::adoption::tests", "ac_refs": ["AC-4", "AC-7"], "depends_on": ["s2", "s3"], "paths": ["crates/epic-orchestrator/src/launch/tests/adoption_test.rs", "crates/epic-orchestrator/src/launch/adoption.rs", "crates/epic-orchestrator/src/disk.rs", "crates/epic-orchestrator/src/launch/agent.rs", "crates/epic-orchestrator/src/launch/attempt.rs", "crates/epic-orchestrator/src/launch/deliver.rs", "crates/epic-orchestrator/src/tests/launch_rig_test.rs", "crates/epic-orchestrator/src/launch/tests/attempt_test.rs"], "input": "Rig live launch with --kind cursor, GitHub fixture, herdr.script(agent.prompt, agent_prompt_stalled); a Job lease request before and after crate::disk::write_issue on a running Issue; a Cursor chat meta.json written with createdAtMs from the record; expect record uncertain + session_id null + job refused, then lease running + job admitted after write_issue, then after launch_issue with Recovery::Reprompt record running, prompt_state acknowledged, session_id = chat uuid, lifecycle_outcome retained, lease running", "model": "inherit"},
  {"id": "s5", "title": "dry-run --replace and --reprompt load the adopted record", "check": "cargo test -p toolu-epic-orchestrator launch::tests::", "ac_refs": ["AC-6"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/tests/launch_test.rs", "crates/epic-orchestrator/src/launch.rs", "crates/epic-orchestrator/src/launch/record.rs"], "input": "Rig state with the #625 record written to issues/comemory-255.json; dry() with Recovery::Replace and Recovery::Reprompt", "model": "inherit"},
  {"id": "s6", "title": "recovery reference and plugin README explain the reprompt repair and the adopted lease", "check": "cargo xtask check-markdown-cli && cargo test -p toolu-cli --test epic_markdown && cargo test -p toolu-cli --test epic_skill", "ac_refs": ["AC-4", "AC-6"], "depends_on": ["s4", "s5"], "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md", "plugins/epic-orchestrator/README.md"], "input": "the real skill Markdown", "model": "inherit"},
  {"id": "s7", "title": "quality gate: every gate step but tests, coverage and bench, plus the changed packages' tests", "check": "R=$(mktemp -d) && env -u npm_config_store_dir TMPDIR=/private/tmp TOOLU_RESOURCE_HOME=$R cargo xtask gate --only gate-change --only fmt --only clippy --only guardrails --only layers --only reach --only deny --only machete --only unused-pub --only hooks --only workflows --only docs-cli --only cli-compat --only jscpd --only rust-quality --only docs --only context-budget --only final-removal --only gate-coverage --only packaging --only ci-paths --only pack && TOOLU_RESOURCE_HOME=$R cargo test -p toolu-epic-orchestrator && TOOLU_RESOURCE_HOME=$R cargo test -p toolu-cli --test epic_markdown --test epic_skill", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7"], "depends_on": ["s1", "s2", "s3", "s4", "s5", "s6"], "paths": ["crates", "docs", "plugins", "tooling"], "input": "the whole branch diff", "model": "inherit"}
]
```

## Critical files
- `crates/epic-orchestrator/src/launch/record.rs`, `launch/tests/record_test.rs`
- `crates/epic-orchestrator/src/launch/adoption.rs` (new), `launch/tests/adoption_test.rs` (new), `crates/epic-orchestrator/src/launch.rs` (module declaration)
- `crates/epic-orchestrator/src/disk.rs`, `src/tests/disk_test.rs`
- `crates/epic-orchestrator/src/launch/agent.rs`, `launch/tests/agent_test.rs`, `launch/attempt.rs`
- `crates/epic-orchestrator/src/tests/launch_test.rs`
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`

## Verification
End to end (s4): a real Cursor-kind agent process on the herdr socket fixture goes
through the #625 path: prompt stall, then an `uncertain` record with null session, then a
refused job, then the engine's `running` write, then the job admitted, then a Cursor chat
on disk, then `--reprompt` recording the session as `acknowledged`/`retained`. Each
negative case keeps today's refusal text (s1, s3) or leaves the resource state
byte-identical (s2). Docs are synchronized in s6. Delivery: scoped commits, `toolu ledger
run <plan> --verify`, `toolu-review:review`, `toolu ledger verdict status` reporting
`overall: ready`, push, PR, then `pr-babysit:babysit`. `gh` uses the stored login
(`env -u GH_TOKEN`), because the exported `GH_TOKEN` is invalid.

## Deviations
- s3/s4: the Cursor chat writer (`Rig::cursor_chat`) and the GitHub-backed live launch
  (`Rig::launch`, previously `attempt_test.rs`'s private `launch_live` body) moved into
  the shared launch rig, so the agent, attempt and adoption tests share them. jscpd
  rejects copied blocks. `launch.rs` makes `digest` `pub(crate)` for the rig's md5.
- s7: the full `cargo xtask gate` cannot pass on this Mac. Its `tests` and `coverage`
  steps fail on the installed Homebrew `toolu` (launcher, `muse_lifecycle` and
  `npm_wrapper` targets), which is unrelated to this diff. `bench` leaves an untracked
  results file. So s7 runs every other step plus the changed crate's tests and the
  `toolu-cli` targets that read the edited skill, the same form as
  `2026-10-24-doctor-retired-plugins.md`. CI runs the full gate.
- Review round 1 (independent reviewer): the adopted shape also loads at `awaiting_merge`,
  where the gate parks it. A plain launch is tested and documented as the repair that
  does not prompt the worker again (AC-7, Jev p=0.91 to keep the shared keep path). The
  best-effort mirror failure is tested through `disk::write_issue`.
