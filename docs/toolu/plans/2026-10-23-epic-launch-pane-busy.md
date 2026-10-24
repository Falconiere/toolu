# Epic launch retries a busy pane — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-epic-launch-pane-busy-design.md   **Topic:** retry `agent_pane_busy` on `agent start`; a not-started refusal never leaves a launch `uncertain`

## Evidence and approach

Inspected `launch/agent.rs` (`ensure_agent` sends `Call::Start` once), `launch/attempt.rs` (`settle` persists every post-reservation failure `uncertain`), `launch/failure.rs` (`Failure` carries herdr's `code`), `herdr_calls.rs` (`result_of` maps a herdr error object to `code`), `effects_launch.rs` (`waits`, extended in #617), the launch rig (`tests/launch_rig_test.rs`) and herdr fixture (`HerdrFixture::script`). `Ctx` is built in `launch.rs`, `verbs_actions/launch.rs`, `verbs_actions/teardown.rs` and the rig. `release_lease` is exported from `toolu_engine::resources::leases`.

Approach per spec: a bounded backoff loop around `agent start` for code `agent_pane_busy` (budget `Ctx::shell_wait`), a not-started settle that releases the lease and drops the stage, and `waits()` matching `agent_pane_busy`.

## Workstream summary

Failure classification → retry loop and `shell_wait` plumbing → not-started settle → engine `wait` → docs → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Retry agent_pane_busy on agent start within Ctx::shell_wait",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::attempt::tests::a_busy_pane_is_retried_until_the_agent_starts",
    "ac_refs": ["AC-1", "AC-4"],
    "paths": [
      "crates/epic-orchestrator/src/launch/agent.rs",
      "crates/epic-orchestrator/src/launch/attempt.rs",
      "crates/epic-orchestrator/src/launch/failure.rs",
      "crates/epic-orchestrator/src/launch.rs",
      "crates/epic-orchestrator/src/verbs_actions/launch.rs",
      "crates/epic-orchestrator/src/verbs_actions/teardown.rs",
      "crates/epic-orchestrator/src/launch/tests/attempt_test.rs",
      "crates/epic-orchestrator/src/launch/tests/agent_test.rs",
      "crates/epic-orchestrator/src/tests/launch_rig_test.rs"
    ],
    "input": "HerdrFixture scripted with one agent.start error {code: agent_pane_busy, message: 'agent target pane … is not an available shell'} (the live #613 refusal), real git origin/checkout, real agent process",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "A not-started refusal past the budget releases the lease and drops the stage",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::attempt::tests::a_pane_busy_past_the_budget_releases_the_launch",
    "ac_refs": ["AC-2"],
    "depends_on": ["S1"],
    "paths": [
      "crates/epic-orchestrator/src/launch/attempt.rs",
      "crates/epic-orchestrator/src/launch/failure.rs",
      "crates/epic-orchestrator/src/launch/agent.rs",
      "crates/epic-orchestrator/src/launch/tests/attempt_test.rs",
      "crates/epic-orchestrator/src/launch/tests/failure_test.rs",
      "crates/epic-orchestrator/src/tests/launch_rig_test.rs"
    ],
    "input": "HerdrFixture scripted with agent_pane_busy replies outlasting a 2 s shell_wait, then a plain relaunch with herdr serving normally",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "The engine launch effect waits on agent_pane_busy",
    "check": "cargo test -p toolu-epic-orchestrator --lib effects_launch::tests::a_busy_pane_refusal_waits",
    "ac_refs": ["AC-3"],
    "paths": [
      "crates/epic-orchestrator/src/effects_launch.rs",
      "crates/epic-orchestrator/src/tests/effects_launch_test.rs"
    ],
    "input": "The live #613 message 'herdr agent.start: agent_pane_busy: agent target pane wE0:p1 is not an available shell' and an unrelated failure message",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Recovery reference says a busy pane is retried, then waits",
    "check": "rg -q 'agent_pane_busy' plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md && cargo xtask check-markdown-cli",
    "depends_on": ["S2", "S3"],
    "paths": [
      "plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md"
    ],
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Crate tests and clippy (full gate runs separately)",
    "check": "cargo test -p toolu-epic-orchestrator && cargo clippy -p toolu-epic-orchestrator --all-targets -- -D warnings",
    "depends_on": ["S1", "S2", "S3", "S4"],
    "paths": [
      "crates/epic-orchestrator",
      "plugins/epic-orchestrator"
    ],
    "model": "inherit"
  }
]
```

## Critical files

- `crates/epic-orchestrator/src/launch/agent.rs` — retry loop; `AgentPlan::shell_wait`.
- `crates/epic-orchestrator/src/launch/failure.rs` — `not_started()`, `not-started` outcome in `details()`.
- `crates/epic-orchestrator/src/launch/attempt.rs` — not-started settle.
- `crates/epic-orchestrator/src/launch.rs`, `verbs_actions/launch.rs`, `verbs_actions/teardown.rs`, `tests/launch_rig_test.rs` — `Ctx::shell_wait`.
- `crates/epic-orchestrator/src/effects_launch.rs` — `waits()`.
- Tests: `launch/tests/attempt_test.rs`, `launch/tests/agent_test.rs`, `launch/tests/failure_test.rs`, `tests/effects_launch_test.rs`.
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`.

## Verification

Red first: S1 and S2 tests fail on `main` (the busy reply settles `uncertain`). Green after: S1–S3 pass on the real socket fixture with real git and agent processes; S2 covers the failure boundary (budget exhausted) and the relaunch without `--replace`; an unrelated message in S3 stays `failed`. Then `cargo test -p toolu-epic-orchestrator`, `cargo xtask gate` through `toolu epic job`, `toolu ledger run <plan> --verify`, `toolu-review:review`, `toolu ledger verdict status` = ready, scoped commit, push, PR with `Closes Falconiere/toolu#613` / `Part of Falconiere/toolu#591`, then `report babysit`.

## Plan review

- S5: 🟡 should-fix (fixed): the check wrapped the full gate in `toolu epic job` inside a ledger check, which already takes a job lease. S5 now runs the crate tests and clippy; the full gate runs through `toolu epic job` before delivery.
- AC coverage: AC-1 → S1, AC-2 → S2, AC-3 → S3, AC-4 → S1; no dangling refs. Jev alignment 0.76.

## Deviations

- S1: the shell wait travels as `AgentPlan::shell_wait` instead of a `Waits` argument to `ensure_agent`, which keeps its signature and its six test call sites unchanged. The spec's Interfaces section was updated to match.
- S1/S2: `launch::PANE_BUSY` names herdr's code once; `effects_launch::waits` matches it.
- S2: when the lease cannot be released, the record also carries `lease_release_error` beside the `uncertain` fallback, so the reason is not lost.
