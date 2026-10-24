# Epic finish goes through the engine and is never undone — Design

**Date:** 2026-10-24   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** `toolu epic finish` against a live engine never relaunches the issue (#619)

## Problem
`toolu epic finish <state_dir> <key> --abandon` runs in its own process and writes only the launch record. The resident engine still holds `stage: running` in memory, so the teardown's own `pane_exited` and `worktree_removed` events reach `schedule::gone`, which arms a checkpoint and a relaunch. The launch effect passes `force: true`, and `require_recoverable_stage` accepts an `abandoned` record. Ten seconds after an abandon, a new agent worked in a recreated worktree (journal 2026-10-24, `falconiere-comemory-398`). When the issue was also removed from the graph, the relaunch failed with `is not in the graph`. Its failure commit then wrote the in-memory `running` over the record, and `reload_epic` kept the stale in-memory issue and its slot until the engine restarted.

## Non-Goals
1. No change to how a paused epic treats `gone`. Pause policy is separate from this issue.
2. No in-process abandon effect. The teardown still runs in the CLI process under the per-issue launch lock.
3. No change to the socket `PROTOCOL` number (2). A client on another number replaces the engine.
4. No change to the `finish` CLI arguments, output document or exit codes.
5. A human's `toolu epic launch` of an abandoned issue keeps today's behaviour. Only the engine's own launches refuse a finished record.

## Architecture
Jev compared three approaches (verb plus guards p=0.94, guards only 0.02, engine-run abandon 0.03). The design takes verb plus guards. The CLI tells a live engine before and after the teardown, and four guards cover the case where the engine is down, restarts, or already armed a launch.

1. **Engine verb.** `verbs_actions/teardown.rs::finish` follows the `release` pattern. When `crate::lock::live(&paths.lock())` holds and the socket exists, and the run is not a dry run, it sends `finish`/`begin` through `client::exchange_retry` before calling `finish_issue`. An `error` reply refuses the finish before anything is torn down. After `finish_issue` returns, success or failure, it sends `finish`/`end` with the stage `begin` returned. The engine handler lives in `dispatch_ops.rs` beside `reply_release`.
   - `begin` sets the issue's in-memory stage to `cleaning`. Every herdr `gone` source already requires `running`, and babysit and stall checks skip `cleaning`. It also clears a waiting `relaunch` and the `follow: Launch` of a pending checkpoint. It writes no record, so a refused finish leaves the record untouched.
   - `end` reads the record's `stage` and adopts it, falling back to the request's `stage` when the record has none. It settles the key's attention items when the stage is `merged` or `abandoned`, journals the result, and pumps. A done issue leaves its slot to refill.
2. **G1, engine launches refuse a finished record.** `LaunchOpts` gains `by: Launcher`. `effects_launch` sets `Engine`, and the CLI leaves `Human`. Under the launch lock, `launch()` refuses a record whose stage is `merged` or `abandoned` with a `Failure::refusal` that starts with the `FINISHED` prefix (`"finished: "`). `effects_launch::run` makes the same check on the record before it reads the graph, so a dropped and abandoned issue settles instead of failing. Either path returns `{"outcome":"settled","stage":S}`. `commit_launch` handles `settled` by closing the launch with note `settled`, adopting `S`, clearing `relaunch`, and marking the batch member `skipped`. It raises no attention item and writes no record.
3. **G2, record writes keep a terminal stage.** `disk::write_issue_fields` does not replace a record stage of `merged` or `abandoned` with a different stage. A failed relaunch therefore never rewrites a terminal record.
4. **G3, `gone` needs a live agent.** `schedule::gone` returns early when `attention::has_live_agent` is false, that is when the stage is `cleaning`, `cleanup-incomplete`, `merged` or `abandoned`. This covers the socket `event` op and future sources.
5. **G4, refresh drops issues that left the graph.** After `load_epic`, `snapshot::reload_epic` removes each issue of that epic whose key is no longer in `graph.json` and that has no pending action. It journals `action drop <key>` with note `not in the graph` and settles the key's attention items. An issue with a pending action is dropped by a later refresh, once the action has committed.

Reused: `client::exchange_retry`, `dispatch::pump_after`, `attention::settle`, `attention::has_live_agent`, `snapshot::issue_path`, `disk::read_value`, and the `release` CLI/engine split.

## Interfaces / Schema
- Socket request: `{"op":"finish","key":K,"step":"begin"}`. Reply: `{"ok":true,"key":K,"tracked":true,"stage":<previous>}`, or `{"ok":true,"key":K,"tracked":false}` for a key the engine does not hold. Refusal: `{"error":"the engine is merging or cleaning up K; wait for it"}` while a `Merge` or `Cleanup` action is pending.
- Socket request: `{"op":"finish","key":K,"step":"end","stage":<previous>}`. Reply: `{"ok":true,"key":K,"stage":<adopted>}`, or `tracked:false`.
- Any other `step`: `{"error":"finish needs step begin or end"}`.
- Journal: `action finish K` with note `begin`, and `action finish K` with note `<adopted stage>`. Also `action drop K` with note `not in the graph`.
- `LaunchOpts { by: Launcher, .. }` with `Launcher::{Human (default), Engine}` (a fourth bool is rejected by clippy). `launch::FINISHED: &str = "finished: "`. Refusal text: `finished: K is abandoned` (or `merged`).
- Launch effect outcome: `{"outcome":"settled","stage":"abandoned"|"merged"}`.

## Failure modes and edge cases
- No live engine (lock not live or no socket): finish runs as today, with no socket traffic.
- An older engine answers `unknown op`: the finish refuses with `toolu epic finish: unknown op (replace the engine with toolu epic engine --replace)`, and nothing is torn down.
- The engine is unreachable after `lock::live` reported it live: the `exchange_retry` error propagates as a refusal, and nothing is torn down.
- `end` fails after the teardown: the finish keeps its result and exit code and adds a stderr warning `engine not told: <err>`. G4 and the stage overlay at the next refresh heal the in-memory stage, because `abandoned` and `merged` outrank `cleaning`.
- The CLI dies between `begin` and `end`: no relaunch happens, because the stage is not `running`. At the next refresh, an issue still `cleaning` with no pending action, whose record holds a live stage (`starting`, `uncertain` or `running`), gets that stage back. That happens only when nobody holds its launch lock; the CLI holds that lock for the whole teardown, so a refresh during a live finish changes nothing. A terminal record stage overlays it as before (review finding, `dispatch_ops::heal_cleaning`).
- `begin` while a `Launch` is pending: the stage becomes `cleaning`. If the launch runs after the teardown, G1 settles it. If it holds the lock first, the finish fails with `launch already in progress` and `end` restores the record's stage.
- `finish_issue` refuses before writing (unmerged PR, ownership mismatch): `end` adopts the unchanged record stage, normally `running`, so the engine resumes watching the live agent.
- Unknown key: `begin` and `end` reply `tracked:false`, and the finish proceeds.
- Dry run: no socket traffic.
- Graph-absent issue with a pending effect: kept until a later refresh, so a commit never targets a missing issue mid-call.

## Acceptance criteria
- **AC-1:** Given an engine opened on a fixture epic whose issue `a` is `running` with worktree `/work`, `finish`/`begin` followed by a `worktree_removed` event for `/work` and a `pane_exited` for its pane arms no pending action, and `a`'s launch count stays 0.
- **AC-2:** After AC-1, with the record rewritten to `stage: abandoned`, `finish`/`end` sets the in-memory stage to `abandoned`, replies `stage: abandoned`, and journals `finish` rows with notes `begin` and `abandoned`. `refill::occupies(a)` is false.
- **AC-3:** Given a launch job for a key whose record is `abandoned`, the in-process launch effect returns `{"outcome":"settled","stage":"abandoned"}` without creating a worktree. Committing it leaves no pending action, no attention item, and the record stage `abandoned`.
- **AC-4:** Given a record with `stage: abandoned` and an in-memory issue at `running`, `disk::write_issue` leaves the record's stage `abandoned`.
- **AC-5:** Given an engine whose epic graph lists issue `a` as `running` and holding a slot, rewriting `graph.json` without `a` and running `reload_epic` removes `a` from the world, journals `drop a`, and leaves the epic with all its slots free.
- **AC-6:** `schedule::note_event(world, key, "gone")` on an issue at `cleaning` or `abandoned` arms nothing and spends no launch.
- **AC-7:** Against an engine served on a real Unix socket, whose issue `a` is `running` in memory and whose record is a never-started attempt (no worktree, agent or pane), the `toolu epic finish <state> a --abandon` verb runs `begin`, the teardown and `end`. The record ends `abandoned`, the served engine's status document shows `a` at stage `abandoned`, and its journal holds no `launch-intent` for `a`. When the engine refuses `begin` (an engine that answers `unknown op`), the verb exits 1 with the engine's error and the record is byte-for-byte unchanged.
- **AC-8:** Committing a `failed` launch outcome with no stage (`a is not in the graph`) for an issue at `running` in memory whose record is `abandoned` leaves the record stage `abandoned`.

## Acceptance evidence
- AC-1/AC-2/AC-6: unit tests in `src/tests/dispatch_ops_test.rs` and `src/tests/schedule_gone_test.rs`, using `source_fix::fixture(true)` with a real `Engine::open` on temp disk state and real journal rows. The failure case is an unknown `step`.
- AC-3: `src/tests/effects_launch_test.rs` with a real temp state dir and graph, and `src/tests/commit_launch_test.rs` for the commit. The boundary case is a `merged` record.
- AC-4: `src/tests/disk_test.rs`, on a real record file. The boundary case: a `cleanup-incomplete` record is still overwritten.
- AC-5: `src/tests/snapshot_test.rs` (or the refresh test) with real `graph.json` files. The boundary case: an issue with a pending action survives the reload.
- AC-7: `src/verbs_actions/tests/teardown_test.rs` (or a sibling test file wired from it), running `serve_with_http` in a thread on a temp resource root with a registered fixture epic. The finish runs through the real verb, and the served engine's status document is read back over the socket. The failure case is a refused `begin`. The full worktree teardown path keeps its coverage in the existing `finish/tests/remove_test.rs`.
- AC-8: `src/tests/commit_launch_test.rs` with a real record file, applying the committed `Step::Issue` through `disk::write_issue`.
- Issue acceptance 3 (live epic): the live-epic check (#591 acceptance: "every sub-issue's repro, run against the merged build") reruns the journal repro on the merged binary.
- Gate: `cargo test -p toolu-epic-orchestrator`, `cargo xtask gate`.

## Documentation impact
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/recovery.md`: the abandon bullet says the engine is told and never relaunches the issue, and that an issue removed from the epic leaves the engine at the next refresh.
- `plugins/epic-orchestrator/README.md`: the `finish` row notes that a live engine is told.
- `docs/epic-orchestrator/README.md`: the `finish` row likewise.
- CLI text is unchanged, so `docs/cli/*` stays as is (`cargo xtask docs-cli` confirms).

## Open Questions
None blocking. The live-epic repro runs after merge under #591's acceptance, because the resident engine on this machine runs the installed binary, not this branch (owner: orchestrator).
