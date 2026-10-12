# Merge queue with mechanical rebase — Design

**Date:** 2026-10-12   **Status:** Draft   **Author:** epic worker (Claude Opus 5.5)   **Topic:** #448: the engine queues, rebases and merges gate-green PRs itself

## Problem

Every merge moves the base, so each other open PR is told `REBASE` at its own
gate: its worker LLM rebases, re-runs the local gate, force-pushes and babysits
again. Measured: 119 rebase rounds for 83 merged issues, 61 rounds for 22
issues in the worst epic. Today the engine does not merge at all: a `ready`
issue arms the `merge` action, which (without `TOOLU_EPIC_SCRIPTS`) is
`deferred` into a `merge-ready` judgment that the orchestrator LLM clears by
running `toolu epic gate --merge` by hand (`merge_bridge.rs`). The repositories
are on personal accounts, where GitHub's native merge queue is not available.

## Non-Goals

1. Stacked merge trains (several PRs tested together).
2. Rewriting the orchestrator skill's engine and gate sections; #452 owns the
   skill and commands for the engine. This issue changes only the worker brief
   sentence about `REBASE` and the gate section's statement of who merges.
3. Removing the `TOOLU_EPIC_SCRIPTS` seam. It stays the test seam; production
   (unset) runs in process.
4. A write API over HTTP, a change to the authorization boundary, or any
   change to `toolu-engine` (core).
5. Changing the gate verdicts. `gate/assess.rs` (#516) is reused unchanged.

## Architecture

**Queue (`queue.rs`, pure).** Membership is derived from engine state: an
issue is queued when its phase is `ready`, its stage is `running` or
`awaiting_merge`, it has a PR and a repository, and its epic is known. Order
within a repository: graph `unblocks` descending, then `queued_at_ms`
ascending, then key. `queued_at_ms` is set when the issue's phase becomes
`ready` (a report or a green babysit tick), cleared when it leaves `ready`, and
persisted as `queued_at` (ISO-8601) in `issues/<key>.json`; a restart reloads
it, so the order survives. The head of a repository is its first entry. One
queue per repository, across epics.

**Head step.** The existing `merge` action is armed only for a repository's
head (`logic::arm_merge`). Triggers: the head's worker reports `ready`; a
green babysit tick moves it to `ready`; a check of the head's PR completes;
the base of its repository moves (`Change::BaseMoved`); the previous head
merged or left (`watch::after_merge` requests immediate PR checks); engine
start (`arm_ready`). A paused epic (or `pause` all) arms and runs nothing, as
today. With the script seam unset, `effects_run` builds an in-process job
(`queue_step.rs`) instead of returning `deferred`:

1. `gate::assess` the PR (unchanged verdicts).
2. `merged` → outcome `merged`. `closed` → `closed`.
3. `fix` → disarm auto-merge if armed → `fix`.
4. `rebase` → disarm auto-merge if armed, then the mechanical rebase (below)
   → `rebased`, `handoff` (conflict or unsafe worktree) or `busy`.
5. `wait` → arm auto-merge pinned to the head (`auto::arm`, `expectedHeadOid`)
   when not armed → `waiting` (with `auto_merge`: `armed` or `unavailable`).
6. `merge` → `merge::do_merge` pinned to the verified head, the repository's
   method (squash preferred), `--admin` only after a protection-only refusal
   → `merged`, `stale` (GitHub refused because the head moved: HTTP 409, or
   a message naming the head SHA), `uncertain` (timeout) or `refused`.
7. A GitHub error before any write → `error`.

The job also runs when the issue left the queue while its auto-merge may be
armed (`Issue.auto_armed`): then it only disarms → `disarmed`.

**Mechanical rebase (`rebase.rs`).** Runs only when the issue is parked:
phase `ready` and the last herdr status of its agent is not `working`
(`Issue.agent_working`, set from `pane_agent_status_changed`). Otherwise the
outcome is `busy` and no git command runs. Steps, each failure noted:

1. Preconditions in the recorded worktree (`issues/<key>.json` `worktree`):
   current branch equals the PR head ref; the head ref is not the base; local
   `HEAD` equals the PR head SHA; no staged or unstaged change to tracked
   files (`git status --porcelain --untracked-files=no` is empty); no rebase
   or merge in progress. A failed precondition → `handoff` with the reason.
2. Fence: `patch_lease` the issue's agent lease (`resource_root`,
   `lease_token` from the record) to stage `rebasing`, then
   `reconcile_resource_jobs` + `active_jobs`; a live job → restore `running`,
   `busy`. An issue with no recorded lease skips the fence.
3. Snapshot: `checkpoint::snapshot` writes `refs/epic-wip/<key>`; a failed
   snapshot (`Snapshot::failed`) → unfence, `busy` (retried next check).
4. `git fetch origin <base>`, then `git rebase origin/<base>`.
5. Rebase fails → `git rebase --abort`; verify `HEAD` equals the old SHA and
   the tracked status is empty → `handoff` ("conflict"). If the abort cannot
   be verified, the outcome is `handoff` with "abort unverified" and the
   attention item names `refs/epic-wip/<key>`.
6. Push: `git push --force-with-lease=refs/heads/<ref>:<old sha> origin
   HEAD:refs/heads/<ref>`. A refused push → reset nothing; `handoff`
   ("push refused") — the worker owns the branch from here.
7. Unfence (`running`) on every path after step 2. Outcome `rebased` with the
   new head SHA. No local gate run: CI on the new head is the check of record.

**Commit (`commit_queue.rs`).** The `merge` outcome document is committed:

| Outcome | Engine change |
|---|---|
| `merged` | today's `applied` path: stage `cleaning`, `cleanup` follows (finish, settle, refill) |
| `rebased` | pending closed (`merge-done` with note), `rebase` journal `{"by":"engine","round":n,"head":sha}`, immediate check of the PR |
| `waiting` | pending closed; `auto_armed` set when armed |
| `busy` | pending closed; `rebase-skipped` journal with the reason; head waits |
| `stale` | pending closed; immediate check of the PR, which gates it again |
| `handoff` | leaves the queue: phase `rebasing`, `queued_at` cleared, `rebase` journal `{"by":"worker","round":n}`, prompt `REBASE` |
| `fix` | leaves the queue: phase `babysit`, `queued_at` cleared, journal `queue-left` with the reasons |
| `closed` | stage `closed`, `pr-closed` attention (as the PR detector does) |
| `uncertain`, `refused` | `deferred = merge`, `failed` attention; never resent until the PR detector sees it merged or the phase changes |
| `error` | pending closed; journal; next check retries |
| `disarmed` | pending closed; `auto_armed` cleared |

Rounds per issue (`Issue.rebase_rounds`) are persisted as `rebase_rounds` in
the record.

**Leaving the queue from babysit.** A babysit tick of a queued issue that
returns `Act` or `Escalate` (a failed check, a new thread or a finding) moves
it to phase `babysit`, clears `queued_at`, and, when `auto_armed`, arms the
`merge` job in disarm mode. A tick that fails (GitHub error or rate limit)
leaves it queued (Jev 0.83).

**Recovery.** An interrupted `merge` reconciles in process when the seam is
unset: `assess` → `merged` (cleanup) or `open` (re-gated at the next check).

**CLI.** `toolu epic queue [--epic <key>]` asks the engine (control op
`queue`) and prints the queue document; without a registered epic it prints
an empty queue. The `planned` verb is removed (it listed only `queue`).

## Interfaces / Schema

- `issues/<key>.json` gains `queued_at` (ISO-8601 string, absent when not
  queued) and `rebase_rounds` (`{"engine": n, "worker": n}`).
- `Issue` gains `queued_at_ms: Option<u64>`, `agent_working: bool`,
  `auto_armed: bool`, `rebase_rounds: (u32, u32)`.
- Queue document (`/api/v1/queue`, op `queue`, `toolu epic queue`):
  `{"schema":1,"repositories":[{"repo":"o/r","items":[{"position":1,"key":"K","epic":"E","pr":7,"unblocks":2,"queued_at":"…","head":true,"last":"waiting"}]}]}`.
  The existing `position` and `key` fields keep their meaning.
- `merge` outcome document: `{"outcome":"<name>","head":"<sha>","reasons":[…],"note":"…","auto_merge":"armed|unavailable|off","admin_used":bool}`.
- Journal records (closed schema, kind `action`): `rebase` (note JSON
  `by`, `round`, `head`), `rebase-skipped` (reason), `queue-left` (reasons).
- Worker prompt for a hand-off: `REBASE` (unchanged text).

## Failure modes and edge cases

- **Worker active** (`working`, or phase not `ready`): `busy`, journal
  `rebase-skipped` "worker active"; no git command, the head waits.
- **Managed job running in the worktree**: lease restored, `busy`.
- **Dirty tracked files, wrong branch, local commits not on the PR**:
  `handoff` with the reason; no git write happened.
- **Untracked file the rebase would overwrite**: git refuses before changing
  anything → abort path → `handoff`.
- **Conflict**: abort; worktree byte-identical (HEAD, index, tracked and
  untracked files) to the snapshot; one `REBASE` prompt; an undelivered prompt
  raises a `rebase` attention item.
- **Push between gate and merge**: the REST merge carries `sha`; GitHub
  refuses with 409 → `stale`; nothing merged; gated again.
- **Base moved while waiting on CI with auto-merge armed**: base watch → arm
  head → gate says `rebase` → disarm before the push → rebase → gate again.
- **Merge timeout**: `uncertain`, never resent; the head stays blocked until
  the PR detector sees it merged or a human acts.
- **Auto-merge disallowed in the repository**: `unavailable`; the engine
  merges itself at the first `merge` verdict.
- **Engine restart mid-step**: the journal intent reconciles in process.
- **Pause**: nothing is armed or run for a paused epic; a running step
  finishes.
- **Two issues readied in the same millisecond**: key order.
- **Issue in two repositories' queues**: impossible; one repo per issue.

## Acceptance criteria

- **AC-1:** The ported merge-gate verdict tests (`gate::assess`, `merge`,
  `auto`, `gate_test`) pass against `toolu epic gate`, verdict for verdict:
  `merge`, `rebase`, `fix`, `wait`, `merged`, `closed`.
- **AC-2:** Given ready issues in two repositories with different `unblocks`
  and ready times, the queue orders each repository by `unblocks` desc, then
  ready time, then key; the order is the same after the engine reloads the
  records; `/api/v1/queue` and `toolu epic queue` print it.
- **AC-3:** Three scripted PRs A, B, C ready together in one repository (real
  git origin and worktrees, GitHub on the HTTPS fixture) merge in queue order
  with exactly two engine rebases (B once, C once) and zero `REBASE` prompts.
- **AC-4:** A conflicting engine rebase leaves the worktree byte-identical to
  its `refs/epic-wip/<key>` snapshot (HEAD, tracked and untracked files) and
  sends the worker `REBASE` exactly once; the issue leaves the queue.
- **AC-5:** A head whose worker is not parked (agent `working`) is never
  rebased: no git command runs, its reflog is unchanged, and the journal holds
  `rebase-skipped`.
- **AC-6:** A push between gate and merge makes the pinned merge call fail
  (409), nothing merges, and the PR is gated again at its next check.
- **AC-7:** `--admin` is used only after a protection-only refusal of a
  gate-green PR, and no merge call is made on a `fix` or `wait` verdict.
- **AC-8:** The journal records each rebase round per issue with `by`
  (`engine` or `worker`) and its round number; the record keeps the counts.
- **AC-9:** Given C waiting on CI with auto-merge armed, a base move disarms
  auto-merge and rebases C again before any merge.
- **AC-10:** Given a queued issue whose babysit tick returns `Act`, or whose
  gate says `fix`, the issue leaves the queue for `babysit` and an armed
  auto-merge is disarmed.
- **AC-11:** `toolu epic queue` exists in `docs/cli/`, the CLI-compat and
  Markdown–CLI gates pass, and the worker brief says the engine rebases clean
  branches itself and sends `REBASE` only for conflicts.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Fixture PR states of `gate_rig_test.rs` | each verdict | paged checks, no workflows | `cargo test -p toolu-epic-orchestrator gate` |
| AC-2 | World with 4 ready issues, graph `unblocks` 0/2/2, ready times; records on disk | order per repo; same after reload | equal time → key | `cargo test -p toolu-epic-orchestrator queue` |
| AC-3 | Temp bare origin, 3 worktrees on branches off `main`, fixture PR states per step | merge order A,B,C; journal 2 `rebase` by engine; 0 prompts | A already up to date | `cargo test -p toolu-epic-orchestrator queue_scenario` |
| AC-4 | Worktree whose branch edits the same line as `main` | abort; bytes = snapshot tree; 1 `REBASE` | untracked file present | `cargo test -p toolu-epic-orchestrator rebase` |
| AC-5 | Head with `agent_working` | `busy`; reflog unchanged; `rebase-skipped` | phase not `ready` | `cargo test -p toolu-epic-orchestrator queue_step` |
| AC-6 | Fixture: merge PUT returns 409 "Head branch was modified" | `stale`; no `merged`; re-armed on next check | — | `cargo test -p toolu-epic-orchestrator queue_step` |
| AC-7 | Fixture `fix`/`wait` states; protection refusal state | no PUT on fix/wait; admin only after protection refusal | `BLOCKED` merge state | `cargo test -p toolu-epic-orchestrator queue_step gate::merge` |
| AC-8 | Scenario of AC-3 and AC-4 | `rebase` records with `by`/`round`; record counts | — | `cargo test -p toolu-epic-orchestrator queue_scenario rebase` |
| AC-9 | Scenario: C armed, `BaseMoved` | disarm mutation sent before push; new head | — | `cargo test -p toolu-epic-orchestrator queue_scenario` |
| AC-10 | Babysit tick `Act` on a ready issue; gate `fix` | phase `babysit`; disarm call | tick failure stays queued | `cargo test -p toolu-epic-orchestrator commit_queue github_engine` |
| AC-11 | `toolu epic queue --help`, `docs/cli/` | docs current; gates pass | no registry → empty queue | `cargo xtask gate --only docs-cli --only cli-compat`; `cargo xtask check-markdown-cli` |

## Documentation impact

- `docs/cli/` regenerated (`toolu epic queue`, `planned` removed) and the
  `commands --json` snapshot accepted.
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`:
  the engine rebases clean branches itself; `REBASE` only for conflicts.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md` gate section:
  the engine merges through the queue; `toolu epic gate` and `toolu epic queue`
  are for manual use and dry runs.
- `AGENTS.md` key-files row for `crates/epic-orchestrator`: the queue.
- Status schema doc (`status_schema.rs`): queue entries are the engine queue.

## Open Questions

None blocking. Decided with Jev: derived queue (0.80), lease fence via
`patch_lease` (1.00), dirty worktree hands off (0.73), keep the script seam
for tests (1.00), only tracked changes count as dirty (0.75), a failed tick
stays queued (0.83), an uncertain merge is held, never resent (0.83).
