# A worker the launch never confirmed: a loadable record, a running lease, a repairable session — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** Falconiere/toolu#625 — a Cursor worker's lease stays `uncertain`, `epic job` refuses it, and `--reprompt`/`--replace` reject its launch record

## Problem
In Falconiere/agavus.io#550 (toolu 8.2.0, herdr 0.9.5, cursor-agent v2026.10.16), a Cursor
launch prompt stalled (`agent_prompt_stalled`). `start()` had already tried to capture the
Cursor chat id once, right after start. Cursor had not written its chat yet, so the record
was left with `session_id: null`. The prompt failure skipped `await_session`, so the attempt
was settled `uncertain`, and its agent lease was set to `uncertain` too. A human pressed
Enter in the pane, and the worker ran normally and reported its phases. Three defects then
blocked every documented recovery:

1. **The engine wrote a record that cannot be loaded.** `logic.rs` `adopt` treats a
   reporting worker as `running`, and `disk::write_issue_fields` writes `stage: running`
   into the record. That record still has `prompt_state: submitting` and
   `session_id: null`. `launch/record.rs` `check_identity` accepts a null session only at
   `starting`/`uncertain`/`replacing` (and a managed `running` record must have a session),
   so `load_launch_record` fails with `session_id may be null only while awaiting capture`.
   `--reprompt` and `--replace` both stop there.
2. **The lease stays `uncertain`.** Adoption writes only the record. The agent lease keeps
   `stage: uncertain`, and managed job admission (`leases.rs` `admit_worktree_job`) accepts
   only `starting`/`running`. So `toolu epic job` refuses the worker's gate with
   `worktree job admission blocked by agent stage uncertain`.
3. **`--reprompt` cannot repair a missing session.** Even with a loadable record, a live
   recorded agent is kept only if `require_recorded_session` finds a recorded id. With
   none, it refuses with `live session identity is missing; inspect before explicit
   --replace`, although Cursor's store now names exactly one chat for that worktree since
   the attempt began.

Since 8.2.0, `main` has fixed two other steps of the reported chain: the busy-pane race
(#622) and the engine rewriting a record that `finish --abandon` had settled (#628).

## Non-Goals
1. No change to managed job admission. An `uncertain` lease still blocks jobs; only a
   worker the engine adopted gets `running`.
2. No relaxation of session identity. A captured session must still be the only fresh
   session for the worktree in the host's own store. Several candidates, or none, still
   refuse.
3. No change to how Cursor, Claude, Codex or Muse sessions are found (`session.rs` capture
   rules and paths stay as they are).
4. Cursor scratch files under `.cursor/tmp` blocking `finish --abandon` are not part of
   this change. That is a separate teardown question.
5. No new CLI flag, verb or record field.

## Architecture
Three narrow changes, one per defect. Jev compared four scopes (validation only;
validation + lease; validation + lease + capture; jobs admit `uncertain`) and chose all
three repairs, p=0.92.

1. **Validation (`launch/record.rs`).** A new `awaiting_capture(record)` holds the "may
   still lack its session" rule. At `starting`/`uncertain`/`replacing` it is true when
   `prompt_state` is not `acknowledged`, as today. At `running` it is true only when
   `prompt_state` is **present** and not `acknowledged`, which is the shape adoption
   leaves. The managed-running check skips records that are awaiting capture. Every record
   that was valid stays valid. An acknowledged `running` record without a session is still
   refused, with the same two messages.
2. **Lease on adoption (`launch/adoption.rs`, new).** `mirror_running(state_dir, key)`
   runs after the engine writes an issue whose stage is `running` (`disk::write_issue`),
   unless the engine's own launch of that issue is pending, which sets the stage when it
   commits.
   It reads the record, which must be `running` and have `resource_root` and
   `lease_token`. It then takes the issue's launch lock without waiting; a launch in
   flight owns the stage, so a held lock means nothing is done. Under the lock it finds
   the agent lease by token. Only a lease at `starting` or `uncertain` with no
   `pendingHost` is patched to `stage: running`, so a lease mid-migration, fenced or
   settled is never touched. The mirror is best effort: a failure leaves the lease as it
   was, the engine's record write still succeeds, and the next write retries. It runs on
   every engine write of a `running` issue, not only on the adopting one, so a record
   adopted by 8.2.0 heals on the worker's next phase report. This reuses `acquire_lock`,
   `read_resource_state`/`leases` and `patch_lease`, as `rebase.rs` does.
3. **Session repair (`launch/agent.rs`).** `require_recorded_session` becomes
   `recorded_session(...) -> Result<Option<String>, Failure>`. With a recorded id, it
   checks it as today and returns `None`. With no recorded id but a recorded
   `session_started_after`, it calls `capture_session(kind, worktree, since, env)`.
   Exactly one fresh session for the worktree is returned as `Some(id)`; otherwise it
   refuses as today. `AgentStart` gains `captured: Option<String>`, which the keep path
   fills. `Attempt::start` writes a captured id to `session_id`. Then the existing
   `deliver` path finishes the record as `running`, `prompt_state: acknowledged`,
   `lifecycle_outcome: retained`, and `persist_attempt` sets the lease to `running` too.

Decisive trade-off: the lease mirror lives in the engine's record write, not in job
admission. This keeps `uncertain` meaningful (ownership unclear, so no jobs), and grants
`running` only on evidence: the worker reported.

## Interfaces / Schema
```rust
// crates/epic-orchestrator/src/launch/record.rs
fn awaiting_capture(record: &Record) -> bool;

// crates/epic-orchestrator/src/launch/adoption.rs (new; tests in launch/tests/adoption_test.rs)
/// Give the agent lease of an engine-adopted `running` record the same stage.
/// `Ok(true)` when the lease was patched.
pub(crate) fn mirror_running(state_dir: &Path, key: &str) -> Result<bool, String>;

// crates/epic-orchestrator/src/launch/agent.rs
pub(crate) struct AgentStart { pub started: bool, pub resumed: bool, pub args: Vec<String>,
  /// A session the record lacked, captured from the host's store for the kept agent.
  pub captured: Option<String> }
```
- `disk::write_issue` calls `crate::launch::adoption::mirror_running` when
  `issue.stage == "running"` and ignores its result.
- Record and lease JSON shapes do not change. The lease's `stage` goes from
  `uncertain`/`starting` to `running`.

## Failure modes and edge cases
- **Acknowledged `running` record, null session:** still refused with `session_id may be
  null only while awaiting capture`. Without `session_id` but with `lease_token`, it is
  refused with `a managed running launch requires session_id`.
- **`running` record without `prompt_state`, null session:** still refused, because it is
  not the adopted shape.
- **Mirror, engine launch pending** (`issue.pending` is a Launch): not called.
- **Mirror, lease needs nothing:** checked before the lock, so a settled lease never takes
  the launch lock and cannot collide with a launch.
- **Mirror, launch lock held:** no change, `Ok(false)`. The in-flight launch writes both
  record and lease.
- **Mirror, lease has `pendingHost`, or a stage other than `starting`/`uncertain`
  (`replacing`, `rebasing`, `running`):** no change, `Ok(false)`.
- **Mirror, record has no lease, a settled stage (`merged`/`abandoned`), or no record:**
  no change, `Ok(false)`.
- **Mirror, resource state unreadable or the patch fails:** `Err`. The caller ignores it,
  the record write stands, the engine continues, and the next `running` write retries.
- **Mirror, lease token not in the resource state** (released): no change, `Ok(false)`.
- **Repair, no `session_started_after`:** refuse with `live session identity is missing;
  inspect before explicit --replace` (unchanged).
- **Repair, zero or several fresh sessions for the worktree, or only a chat older than
  `session_started_after`:** same refusal. The failed attempt settles `uncertain` as any
  post-reservation refusal does today. A later engine `running` write mirrors the lease
  again.
- **Repair, session store unreadable** (not `NotFound`): the `capture_session` error
  propagates as an uncertain failure (unchanged semantics).
- **Resume path** (`verify_started`): a resume requires a recorded id beforehand
  (`captured session ID missing`), so the capture branch is never reached there.
- **`--replace` on the adopted record:** loads, passes `require_recoverable_stage`
  (`running`), and goes through the existing migration path.

## Acceptance criteria
- **AC-1:** Given the #625 record shape `{"stage":"running","prompt_state":"submitting",
  "lifecycle_outcome":"uncertain","session_id":null,"lease_token":"lease-1",
  "resource_root":"/r","pane_id":"p","worktree":"/w"}`, `load_launch_record` returns it
  unchanged. `{"stage":"running","prompt_state":"acknowledged","session_id":null}` and
  `{"stage":"running","session_id":null}` are still refused with `session_id may be null
  only while awaiting capture`, and an acknowledged managed `running` record without a
  session is still refused with `a managed running launch requires session_id`.
- **AC-2:** Given a real resource root with an agent lease at `uncertain` bound to worktree
  W, and an issue record at `running` holding that lease, `mirror_running` returns
  `Ok(true)` and the lease's stage becomes `running`. A managed job lease request for W
  (`acquire_lease` with `LeaseKind::Job`) is then admitted, where before it failed with
  `worktree job admission blocked by agent stage uncertain`.
- **AC-3:** `mirror_running` returns `Ok(false)` and leaves the resource state
  byte-identical when the launch lock is held, the lease has `pendingHost`, the lease
  stage is `replacing`, the record is `abandoned`, the record has no lease, or `disk::write_issue` runs while
  the engine's own launch of the issue is pending.
- **AC-4:** A live Cursor launch on the herdr fixture whose first `agent.prompt` answers
  `agent_prompt_stalled` leaves the record `uncertain` with `session_id: null`, and a
  job in its worktree is refused. The engine's `disk::write_issue` then writes `running`
  and the lease becomes `running`, so the job is admitted. With a Cursor chat
  `<HOME>/.cursor/chats/<md5(worktree)>/<uuid>/meta.json` (`cwd` = worktree,
  `createdAtMs` ≥ `session_started_after`) on disk, `launch_issue` with `--reprompt` then
  succeeds. The record ends `running`, `prompt_state: acknowledged`,
  `session_id: <uuid>`, `lifecycle_outcome: retained`, and the lease is `running`.
- **AC-5:** For the same live agent with no chat, with two fresh chats, or with only a
  chat whose `createdAtMs` is before `session_started_after`, `ensure_agent` refuses with
  `live session identity is missing; inspect before explicit --replace` and captures
  nothing.
- **AC-6:** A dry-run `--replace` and a dry-run `--reprompt` launch over the AC-1 record
  succeed, where before they failed with `invalid launch record …: session_id may be null
  only while awaiting capture`.

## Acceptance evidence
- **AC-1:** Real JSON files in a temp dir read by `load_launch_record`. The boundary cases
  are the acknowledged and missing `prompt_state` variants. Check:
  `cargo test -p toolu-epic-orchestrator launch::record::tests`.
- **AC-2:** A real `TOOLU_RESOURCE_HOME` temp root. The lease is acquired with
  `acquire_lease`, the stage is set with `patch_lease`, and the record is written with
  `write_record`. Admission is checked with the real `acquire_lease` for a Job lease
  before and after. Check: `cargo test -p toolu-epic-orchestrator launch::adoption::tests`.
- **AC-3:** Same real root. The launch lock is held with `acquire_lock`, `pendingHost` is
  set with `prepare_agent_migration`, and the resource file bytes are compared before and
  after. Check: `cargo test -p toolu-epic-orchestrator launch::adoption::tests`.
- **AC-4:** The herdr socket fixture with a real Cursor-kind agent process and a real git
  worktree (the `Rig`). The stall comes from `herdr.script("agent.prompt",
  agent_prompt_stalled)`. The Cursor chat is written in Cursor's real on-disk format;
  `createdAtMs` comes from the record. The engine write is `crate::disk::write_issue` on
  an `Issue` at `running`. Check: `cargo test -p toolu-epic-orchestrator
  launch::adoption::tests`.
- **AC-5:** The `Rig` live agent, with chat directories written in Cursor's format (none,
  two, one stale), through `ensure_agent`. Check: `cargo test -p toolu-epic-orchestrator
  launch::agent::tests`.
- **AC-6:** The `Rig` state with the AC-1 record written, through the existing `dry()`
  helper with `Recovery::Replace` and `Recovery::Reprompt`. Check: `cargo test -p
  toolu-epic-orchestrator launch::tests::`.
- The whole change: `cargo xtask gate` before push.

## Documentation impact
`plugins/epic-orchestrator/README.md` (launch ownership paragraph) and
`plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`, Relaunch
section: a record whose launch never captured its session (`session_id: null`, the worker
working) is repaired with `--reprompt`. That records the one session the host wrote for
the worktree since the attempt began. The engine gives a reporting worker's lease
`running`, so its gate runs. No CLI contract, flag or generated-docs change.

## Open Questions
None blocking. Whether `finish --abandon` should tolerate Cursor's `.cursor/tmp` scratch
files is out of scope (Non-Goal 4). Owner: maintainer; follow-up issue if wanted.
