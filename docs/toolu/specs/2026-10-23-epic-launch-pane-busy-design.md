# Epic launch retries a busy pane — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Claude (epic worker for #613)   **Topic:** `agent_pane_busy` on `agent start` is retried and never leaves a launch `uncertain`

## Problem

`toolu epic launch` creates the worktree, then calls `herdr agent start` on the
new root pane at once. herdr 0.9.5 refuses with `agent_pane_busy` while the
pane's shell (fish) is not yet at its prompt. That refusal proves nothing
started, yet `Attempt::settle` (`crates/epic-orchestrator/src/launch/attempt.rs`)
persists every post-reservation failure as stage `uncertain`, so the engine and
a plain relaunch refuse until a human inspects and runs `--replace` (#613).

## Non-Goals

1. No probing of the pane's process before `agent start`; herdr's own refusal is the readiness signal.
2. No change to any other failure: timeouts, stalled prompts and every other error after reservation stay `uncertain`.
3. No new record stage, and no change to the stage lists in `launch/record.rs`, `finish/record.rs` or `graph/classify.rs`.

## Architecture

1. **Retry.** `ensure_agent` (`launch/agent.rs`) sends `agent start`; while herdr
   answers code `agent_pane_busy`, it sleeps and sends it again, starting at
   250 ms and doubling up to 2 s, until a shell-wait budget is spent. The
   budget is a new `Ctx::shell_wait` (live: 30 s). A sleep never runs past the remaining budget; once the budget is spent the last refusal is returned. herdr refused before starting
   anything, so a resend cannot start a second agent. Each retry is logged
   (`herdr agent.start: pane <pane> busy; retry in <ms> ms`).
2. **Not-started settle.** A `Failure` gains `not_started()`: true when its
   herdr code is `agent_pane_busy`. When the retries run out, `settle` does not
   persist `uncertain`. It releases the attempt's agent lease
   (`toolu_engine::resources::leases::release_lease`), removes `stage`,
   `lease_token` and `attempt_id` from the record, keeps the failure details
   (`lifecycle_error`, `lifecycle_outcome: not-started`, `native_error`) and
   writes it. A record without a stage is a launch that never ran: it holds no
   slot and a plain launch proceeds. The worktree stays; the next attempt
   reuses it through `ensure_worktree`.
3. **Engine.** `waits()` (`effects_launch.rs`) also matches `agent_pane_busy`,
   so the engine's launch effect ends `wait` and re-arms the issue with no
   `failed` attention item (the #617 path).

Decisive trade-off: dropping the stage and lease (chosen; Jev p=0.91) versus a
new `refused` stage that would touch four stage lists, or keeping the lease
with a restored snapshot, which would hold a slot for an agent that does not
exist.

## Interfaces / Schema

- `Ctx { …, shell_wait: Duration }`; `Ctx::live` sets 30 s; the test rig sets 2 s.
- `ensure_agent(herdr, plan, log, waits: Waits)` with `Waits { session: Duration, shell: Duration }`.
- `Failure::not_started(&self) -> bool`.
- `Failure::details()` writes `lifecycle_outcome: "not-started"` when `not_started()`.
- Record after an exhausted retry: no `stage`, `lease_token` or `attempt_id`;
  `lifecycle_error: "herdr agent.start: agent_pane_busy: …"`,
  `lifecycle_outcome: "not-started"`, `native_error.code: "agent_pane_busy"`,
  `native_error.uncertain: false`; identity fields (`worktree`, `pane_id`,
  `workspace_id`) kept.

## Failure modes and edge cases

- **Busy, then accepted:** the start proceeds; the record goes `starting` → `running` and is never written `uncertain`.
- **Busy past the budget:** the launch fails with the last herdr message; lease released; stage removed; engine outcome `wait`.
- **Lease release refused** (active jobs in the worktree): settle falls back to the previous behavior, persisting `uncertain` with the original failure details, because ownership could not be returned safely.
- **Other herdr errors on start** (timeout, unknown code, transport failure): not retried; `uncertain` as today.
- **CLI fallback:** a busy refusal through the herdr CLI carries the same JSON error object, so its code is parsed by `result_of`; a plain-text CLI failure has no code and is not retried.
- **Fields kept on a not-started settle:** `ref`, `key`, `kind`, `launches`, `session_id`, `prompt_state`, `attempt_started_at` and the worktree identity stay; `launches` is not incremented.
- **Dry run:** never calls herdr; unchanged.
- **Relaunch of a lost agent (`--replace`, record `running`):** a busy refusal removes the stage; the next engine launch takes the resume path with the recorded session.

## Acceptance criteria

- **AC-1:** Given the herdr fixture answering `agent.start` once with `agent_pane_busy` and then serving it, a full `launch_issue` ends with record stage `running`, the issue's agent live, and no `uncertain` settle (the record carries no `lifecycle_error`).
- **AC-2:** Given the herdr fixture answering `agent.start` with `agent_pane_busy` for longer than the shell-wait budget, `launch_issue` fails with the `agent_pane_busy` message; the record has no `stage`, `lifecycle_outcome` is `not-started`, no agent lease remains, and an immediate plain `launch_issue` (no `--replace`, no `--reprompt`) with herdr serving normally ends `running`.
- **AC-3:** Given a launch failure message containing `agent_pane_busy`, the engine launch effect's `waits()` classifies it as `wait`.
- **AC-4:** On herdr 0.9.5, a launch into a freshly created worktree whose shell is still starting starts the agent within that one launch attempt (no second launch, no `--replace`).

## Acceptance evidence

- **AC-1:** Real herdr socket fixture (`HerdrFixture::script("agent.start", Scripted::Error{code:"agent_pane_busy",…})`), real git checkout and worktree, real agent process. Check: `cargo test -p toolu-epic-orchestrator launch::attempt::tests::a_busy_pane_is_retried_until_the_agent_starts` — asserts stage `running`, two `agent.start` calls in `HerdrFixture::calls()`, `lifecycle_outcome: started`, and no `lifecycle_error` in the record (`settle` writes it whenever it persists `uncertain`, and a successful attempt never removes it).
- **AC-2:** Same fixture scripted with enough busy replies to outlast a 2 s budget, then a second launch. Check: `cargo test -p toolu-epic-orchestrator launch::attempt::tests::a_pane_busy_past_the_budget_releases_the_launch` — asserts record fields, `rig.leases() == 0`, and the second launch ends `running`. Boundary: the failure path.
- **AC-3:** Check: `cargo test -p toolu-epic-orchestrator effects_launch::tests::a_busy_pane_refusal_waits`.
- **AC-4:** Real input: the refusal captured live on herdr 0.9.5 in #613, `herdr agent.start: agent_pane_busy: agent target pane wE0:p1 is not an available shell`, which is the `result_of` format `herdr <method>: <code>: <message>` and so proves the live code string is exactly `agent_pane_busy`. AC-1 replays that refusal over the real socket protocol and shows the same attempt starts the agent. A live engine launch on herdr 0.9.5 is not run from this worktree (it needs a scratch epic and GitHub issues); the first engine launch after merge exercises it.

## Documentation impact

`plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`:
one sentence that an `agent_pane_busy` refusal is retried, then waits, and is
never uncertain.

## Open Questions

None — the exhaustion behavior was decided from the issue's Expected section
and code evidence (see Architecture).

## Spec review

- Acceptance criteria: 🔴 blocker (fixed): the issue's live herdr 0.9.5 acceptance had no AC. Added AC-4 with the captured live refusal as real input.
- Architecture: 🟡 should-fix (fixed): the backoff's behavior at the budget edge was unstated. A sleep is capped to the remaining budget.
- Failure modes: 🟡 should-fix (fixed): the fields a not-started settle keeps were unstated. Listed them.

Jev (spec review): coverage 0.50 and gap 0.67 before the fixes above; findings verified against the issue body and `launch/attempt.rs`.
