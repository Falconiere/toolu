# Codex session capture across local-date folders — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** Falconiere Barbosa (epic worker)   **Topic:** Falconiere/toolu#620 — find Codex sessions written under the local-date folder

## Problem
Codex writes each rollout under its **local** date:
`$CODEX_HOME/sessions/2026/10/23/rollout-2026-10-23T22-33-59-<id>.jsonl` at 22:33 in
UTC-3, which is already 2026-10-24 in UTC. `launch/session.rs` builds the Codex search
folders from the **UTC** dates of `since` and now (`day_dir`), so from 21:00 to midnight
in UTC-3 it searches only `sessions/2026/10/24/`, finds nothing, and `await_session` fails
with `cannot attest codex session identity; inspect the owned pane before retry`. Every
late-day Codex launch ends `uncertain` and the brief is never sent.

## Non-Goals
1. No new time-zone dependency (jiff/chrono) and no `unsafe` `localtime_r`; the workspace
   forbids unsafe code and cargo-deny gates dependencies.
2. No change to the match rule: still exactly one fresh `session_meta` whose `cwd` is the
   worktree, otherwise no answer.
3. No change to `metrics/tokens.rs`: its Codex reader (`find_file(sessions, "<id>.jsonl", 4)`)
   recurses the whole tree to depth 4 and has no date assumption (checked).
4. Claude and Cursor capture are untouched.

## Architecture
Every real UTC offset lies within −12h..+14h, so the local date of an instant `t` is always
the UTC date of `t − 1 day`, `t`, or `t + 1 day`. Codex capture searches the union of those
three UTC day folders for both `since` and now (at most six distinct folders, usually three
or four). Muse capture already does exactly this for `since` (`muse_session.rs`
`session_dirs`, "the day folders may be local time"); the day-window computation moves
into one shared helper in `session.rs` that both hosts use, with `DAY_MS` moving with it.

Decisive trade-off: scanning neighbour folders costs a few extra `read_dir` calls per poll,
but needs no knowledge of the machine's time zone, so it is correct under any `TZ`, DST
change, or host that writes in UTC. A wrong-day file cannot be accepted by mistake: the
existing freshness (`mtime >= since`), `cwd` and single-candidate checks still apply.

Rejected: computing the true local date (needs a new dependency or unsafe code);
recursively scanning all of `sessions/` (an established Codex home exceeds `MAX_FILES`
quickly, so capture would always answer `None`). Jev choice over these: `neighbor`, p=1.0.

## Interfaces / Schema
In `crates/epic-orchestrator/src/launch/session.rs`:

```rust
pub(super) const DAY_MS: u64 = 86_400_000;

/// The `sessions/<yyyy>/<mm>/<dd>` folders that can hold a session written at any
/// of `instants` (epoch ms) in any time zone: the UTC dates one day either side.
pub(super) fn day_dirs(base: &Path, instants: &[u64]) -> BTreeSet<PathBuf>;
```

- Codex `transcripts()` roots become `day_dirs(&home, &[since, now])`.
- Muse `session_dirs()` uses `day_dirs(root, &[since])` (behaviour unchanged) and drops its
  private `DAY_MS`.
- `day_dir` stays as the single-date building block. No CLI, state, or file format changes.

## Failure modes and edge cases
- **Folder absent** (any of the window days): skipped as today (`NotFound` → empty).
- **Unreadable folder** (permission error): propagates as `Err` from `capture_session`,
  unchanged.
- **Older session in a neighbour folder** (yesterday's run in the same worktree): rejected
  by `mtime >= since`, unchanged.
- **Two fresh sessions for the worktree across folders**: more than one candidate → `None`,
  unchanged ambiguity rule.
- **Window crosses a month or year boundary**: `day_dir` derives each folder from the full
  UTC ISO date of `t ± DAY_MS`, so `2026/10/31 → 2026/11/01` and `2026/12/31 → 2027/01/01`
  are correct.
- **`since` near the epoch**: `saturating_sub` keeps the window non-negative.
- **More than `MAX_FILES` files across the window**: `jsonl_files` returns `None`, unchanged.

## Acceptance criteria
- **AC-1:** Given a Codex session for the worktree written (fresh mtime) under the UTC
  date folder one day **before** `since` — the folder Codex uses in UTC-3 after 21:00 local
  — `capture_session(Codex, …)` returns that session id.
- **AC-2:** Given the same session written under the folder one day **after** `since`
  (a host east of UTC, e.g. UTC+14, before UTC midnight), capture returns that id.
- **AC-3:** Given two fresh sessions for the worktree, one in the previous-day folder and one
  in the `since` folder, capture returns `None` (ambiguity is still refused).
- **AC-4:** Muse capture keeps its existing behaviour: the existing `muse_session_test.rs`
  cases pass unchanged after the shared-helper refactor.
- **AC-5:** On herdr 0.9.5, a Codex worker launched between 21:00 and midnight in UTC-3
  records `running` with its session on the first attempt.

## Acceptance evidence
- **AC-1:** real files in a temp `CODEX_HOME`: a `session_meta` JSONL at
  `sessions/<UTC date of since − 1 day>/<id>.jsonl`, written now (fresh). Expected
  `Ok(Some(id))`; before the fix `Ok(None)` (red). Check:
  `cargo test -p toolu-epic-orchestrator launch::session::tests`.
- **AC-2:** same fixture under `<UTC date of since + 1 day>`. Expected `Ok(Some(id))`;
  red before the fix. Same check.
- **AC-3:** two real files, previous-day and `since`-day folders, both fresh with the
  worktree `cwd`. Expected `Ok(None)`. Same check.
- **AC-4:** existing real-file Muse cases. Check:
  `cargo test -p toolu-epic-orchestrator launch::muse_session::tests`.
- **AC-5:** two parts. (a) Pre-merge live layout probe with the real Codex CLI: run
  `TZ=Pacific/Kiritimati codex exec … </dev/null` (UTC+14, so its local date is ahead of
  UTC at the probe time) in a temp directory and confirm the new rollout lands in the
  local-date folder, not the UTC one — the layout AC-1/AC-2 fixtures reproduce. Result
  goes in the PR body. (b) Post-merge, the epic orchestrator's next late-day Codex launch
  on herdr 0.9.5 (the issue's repro) records `running` with its session; owner: the epic
  #591 run, recorded on the epic as evidence. The engine-level launch needs the resident
  engine and herdr, which this worker does not drive.

## Documentation impact
None — internal capture behaviour; no command, flag, config, or documented path changes.
The `session.rs` and `muse_session.rs` doc comments are updated to say the day folders may
be local time.

## Open Questions
None blocking. AC-5 is verified live after merge by the epic orchestrator (owner: epic
#591 run), as the issue's own acceptance requires a real late-day launch.

## Spec review

**Status:** Approved

- Acceptance evidence: 🔵 consider: AC-5(b) is post-merge and engine-driven; owned by the epic #591 run, as the issue's acceptance requires a real late-day launch. Pre-merge live layout probe AC-5(a) covers the cause. No change.
- No blockers: problem, non-goals, single approach, interfaces, failure modes, real-file evidence, and documentation impact are concrete.
