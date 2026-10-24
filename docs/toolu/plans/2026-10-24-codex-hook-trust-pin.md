# Codex hook trust that sticks for epic workers (#626) — Plan

**Date:** 2026-10-24   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-24-codex-hook-trust-pin-design.md   **Topic:** pin toolu's Codex marketplace to the release tag; report a hook-review screen as needs-human

## Evidence and approach

Inspected: `crates/plugins/src/host/adapter.rs` (Codex `marketplace add` has no
ref), `verbs/prepare.rs` (Muse already pins its checkout to `v<version>`),
`dispatch.rs::driver` (builds `Driver`), the `Sandbox` host stubs in
`src/tests/support_test.rs`; `crates/toolu/src/doctor/inventory.rs` (Codex
inventory, `warn plugins` notices); `crates/epic-orchestrator/src/launch.rs`
(`go` then `settle`), `launch/failure.rs` (`details()` outcome),
`effects_launch.rs` and `commit.rs::commit_launch` (failed launch →
`failed` attention). Codex 0.164.0 behaviour was verified live in a private
`CODEX_HOME` (brainstorm). The workspace `toml` crate is already a dependency
of `xtask`.

Approach as in the spec: installer pins Codex and re-pins an unpinned
`Falconiere/toolu` marketplace (restore on failure, re-add plugins); doctor
warns on an unpinned or stale pin; a launch failure whose pane shows Codex's
hook review becomes `needs-human`.

## Workstream summary

adapter argv and list parser → pin + prepare + install/update re-add → doctor
notice → launch screen detection through to the engine attention → docs →
live evidence → full gate.

## Steps (machine-readable)

```json
[
  {"id": "s1", "title": "Codex adapter argv for a pinned add, marketplace list and remove; parse marketplace list JSON", "check": "cargo test -p toolu-plugins host::", "ac_refs": ["AC-2", "AC-3"], "paths": ["crates/plugins/src/host/adapter.rs", "crates/plugins/src/host/codex.rs", "crates/plugins/src/host/tests/adapter_test.rs", "crates/plugins/src/host/tests/codex_test.rs"], "input": "`codex plugin marketplace list --json` output captured from Codex 0.164.0 on this machine (git and local sources)", "model": "inherit"},
  {"id": "s2", "title": "install and update pin the Codex marketplace to v<version>, re-pin an unpinned toolu git source, restore on failure, re-add installed plugins after a re-pin", "check": "cargo test -p toolu-plugins verbs::", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"], "depends_on": ["s1"], "paths": ["crates/plugins/src/verbs.rs", "crates/plugins/src/verbs/pin.rs", "crates/plugins/src/verbs/prepare.rs", "crates/plugins/src/verbs/install.rs", "crates/plugins/src/verbs/update.rs", "crates/plugins/src/dispatch.rs", "crates/plugins/src/cursor.rs", "crates/plugins/src/verbs/tests", "crates/plugins/src/tests"], "input": "a scripted `codex` host holding marketplace state in a file and replaying Codex 0.164.0's verbatim refusal `marketplace 'toolu' is already added from a different source; remove it before adding this source`; states: none, unpinned git, pinned to the tag, local source; boundary: the pinned add fails after the remove (restore runs), and the restore fails too (both named); a dry run runs no marketplace command", "model": "inherit"},
  {"id": "s3", "title": "doctor warns plugins when the Codex toolu marketplace is unpinned or pinned to another tag", "check": "cargo test -p toolu-hub doctor::", "ac_refs": ["AC-5"], "paths": ["crates/toolu/Cargo.toml", "crates/toolu/src/doctor.rs", "crates/toolu/src/doctor/codex_pin.rs", "crates/toolu/src/doctor/inventory.rs", "crates/toolu/src/doctor/tests/codex_pin_test.rs"], "input": "config.toml text copied from this machine's `[marketplaces.toolu]` (no ref), the same with ref v8.1.4 and with the binary's tag; boundary: absent file, unparsable file, local source", "model": "inherit"},
  {"id": "s4", "title": "a launch failure whose pane shows Codex's hook review becomes approval_pending, lifecycle_outcome needs-human, effect outcome needs-human and a needs-human attention item", "check": "cargo test -p toolu-epic-orchestrator launch:: && cargo test -p toolu-epic-orchestrator effects_launch && cargo test -p toolu-epic-orchestrator commit", "ac_refs": ["AC-6"], "paths": ["crates/epic-orchestrator/src/launch.rs", "crates/epic-orchestrator/src/launch/screen.rs", "crates/epic-orchestrator/src/launch/failure.rs", "crates/epic-orchestrator/src/launch/tests/screen_test.rs", "crates/epic-orchestrator/src/launch/tests/failure_test.rs", "crates/epic-orchestrator/src/effects_launch.rs", "crates/epic-orchestrator/src/tests/effects_launch_test.rs", "crates/epic-orchestrator/src/commit.rs", "crates/epic-orchestrator/src/tests/commit_launch_test.rs"], "input": "pane text captured from a real Codex 0.164.0 hook review and the TUI singular and plural strings; boundary: tail without the screen, tail with a provider-limit line, a failed pane read keeps the original failure, a not-started refusal is not read", "model": "inherit"},
  {"id": "s5", "title": "installer doc, preflight and recovery references describe the pin, its doctor notice and the needs-human approval launch", "check": "cargo xtask check-markdown-cli && cargo test -p toolu-cli --test epic_markdown && cargo test -p toolu-cli --test epic_skill", "ac_refs": ["AC-1", "AC-5", "AC-6"], "depends_on": ["s2", "s3", "s4"], "paths": ["docs/cli/installer.md", "plugins/epic-orchestrator/skills/epic-orchestrator/references/preflight.md", "plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md", "plugins/epic-orchestrator/README.md"], "input": "the real Markdown files", "model": "inherit"},
  {"id": "s6", "title": "live evidence on Codex 0.164.0: re-pin keeps plugins and trust, foreign source untouched, two fresh-worktree sessions after one trust show no review, untrusted home shows the review screen", "check": "test -s docs/toolu/evidence/2026-10-24-codex-hook-trust.md && grep -q 'ref = \"v8.2.0\"' docs/toolu/evidence/2026-10-24-codex-hook-trust.md && grep -q 'hooks need review before they can run' docs/toolu/evidence/2026-10-24-codex-hook-trust.md", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-7"], "depends_on": ["s2", "s4"], "paths": ["docs/toolu/evidence/2026-10-24-codex-hook-trust.md"], "input": "real codex 0.164.0 in herdr panes, private CODEX_HOME with auth linked, two `git worktree add` toolu worktrees, target/debug/toolu", "model": "inherit"},
  {"id": "s7", "title": "full quality gate", "check": "toolu epic job -- cargo xtask gate", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"], "depends_on": ["s1", "s2", "s3", "s4", "s5"], "model": "inherit"}
]
```

## Critical files

- `crates/plugins/src/host/{adapter,codex}.rs`, `crates/plugins/src/verbs/{pin,prepare,install,update}.rs`, `crates/plugins/src/verbs.rs`, `crates/plugins/src/dispatch.rs`
- `crates/toolu/Cargo.toml`, `crates/toolu/src/doctor/{codex_pin,inventory}.rs`, `crates/toolu/src/doctor.rs`
- `crates/epic-orchestrator/src/launch.rs`, `crates/epic-orchestrator/src/launch/{screen,failure}.rs`, `crates/epic-orchestrator/src/{effects_launch,commit}.rs`
- `docs/cli/installer.md`, `plugins/epic-orchestrator/skills/epic-orchestrator/references/{preflight,recovery}.md`, `plugins/epic-orchestrator/README.md`
- `docs/toolu/evidence/2026-10-24-codex-hook-trust.md`

## Verification

- Unit and crate tests per step with captured Codex 0.164.0 output.
- Live: `CODEX_HOME=<private> target/debug/toolu plugins update --host codex`
  re-pins an unpinned install; `config.toml` before/after diff shows `ref`
  added and `[plugins.*]`/`[hooks.state]` kept. Two herdr panes in fresh
  worktrees start Codex with no review after one trust. A fresh private home
  shows the review screen; its `herdr agent read` text feeds the s4 fixture.
- Failure paths: restore after a failed pinned add (s2), foreign source left
  untouched (s2, s6), no screen keeps `failed` (s4).
- Docs: installer, preflight, recovery updated and checked by
  `check-markdown-cli`.
- `toolu epic job -- cargo xtask gate` green.

## Delivery

After s7: scoped commits on `feat/626-epic-launch-codex-workers-in`; `toolu
ledger run docs/toolu/plans/2026-10-24-codex-hook-trust-pin.md --verify`;
`toolu-review:review` (version 2 state); `toolu ledger verdict status` prints
`overall: ready`; push; open the PR against `main` with body `Closes
Falconiere/toolu#626` / `Part of Falconiere/toolu#591`; report `pr-open`, then
`babysit` (the engine babysits; no `/pr-babysit:babysit` here, per the brief).
Prerequisites: `gh api user` succeeds and the branch is not `main`.

## Plan review (2026-10-24)

Round 1, Jev `aligned` 0.27, gap `delivery` 0.69:

- Verification: 🔴 blocker: delivery steps (final verify, review, verdict, push, PR) absent. Added `## Delivery`. Fixed.
- s2/s4: 🟡 should-fix: restore-also-fails, dry-run, failed pane read and not-started boundaries not named in inputs. Added. Fixed.

Round 2 (Approved), Jev `aligned` 0.37, gap `none` 0.42 / `live_feasibility` 0.43:

- s6: 🟡 should-fix: live TUI evidence might not be producible. De-risk by running
  its Codex-only half (manual pinned marketplace, review screen capture, two
  fresh-worktree sessions) before s1, so s4 gets its captured fixture and a
  failure there returns to the spec before code. The installer half runs after s2.
