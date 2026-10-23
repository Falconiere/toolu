# Scoped and restart-safe `toolu epic wait` — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Falconiere Barbosa (epic worker)   **Topic:** Falconiere/toolu#588, part of #591

## Problem

Several epics share one resident engine. `toolu epic wait`, the
epic-orchestrator skill's main loop, hands out the oldest undelivered attention
item of *any* epic. An orchestrator authorized for one epic gets items it may
not touch (on CodaSignal/toolu.sh#81, three `failed` items of a paused epic came
first, one per call) and can't tell them apart without parsing each one.

After an engine restart the same items come back. `attention::take` marks an
item `delivered` in memory only; the rebuild from the journal
(`attention::observe`) never learns that a waiter already received it, because
the `wait-reply` journal line carries the item's key and kind but not its token.

## Non-Goals

1. No session-inferred default scope: nothing links a `wait` call to the
   `start` call of the same orchestrator session, so `wait` with no flag keeps
   today's every-epic behaviour (Jev: explicit flag 0.66 over a one-epic
   default 0.34; with one registered epic both behave the same).
2. No per-waiter cursor beyond the existing per-item `delivered` flag: an item
   is still delivered once, to the first waiter whose scope matches it.
3. No change to `status`, `queue`, `ack`, `answer`, reminders or the journal
   schema (the `token` field already exists on every record).
4. No exactly-once guarantee across a crash between sending a reply and
   appending its journal line: that window still replays the item
   (at-least-once), as today.

## Architecture

1. **Scope.** `toolu epic wait` gains `--epic <EPIC>`, an epic key or a trailing
   issue number, matched like `toolu epic status <EPIC>`
   (`key == sel || key.ends_with("-sel")`). The CLI sends
   `{"op":"wait","max_seconds":N,"epic":"<sel>"}`. The engine resolves the
   selector against the registered epics (`world.epics`); an unknown selector
   replies `{"error":"unknown epic: <sel>"}` at once instead of waiting forever.
   A `Waiter` keeps its resolved epic (`Option<String>`).
   `attention::take(world, scope)` returns the oldest undelivered item whose
   `epic` equals the scope **or is empty** (an engine-wide item, which concerns
   every epic; Jev 0.65 for this over hiding them from scoped waiters).
   `satisfy` walks waiters in arrival order and gives each the oldest item its
   scope matches, so a waiter for epic A no longer blocks behind an item only
   epic B's waiter can take.
2. **Restart.** `wait_journal::replied` writes the delivered item's token into
   the `wait-reply` record's `token` field. Every reply that hands out a
   judgment is journaled, including a `--max-seconds 0` reply (today only
   blocking replies are); a `--max-seconds 0` reply without a judgment stays
   unjournaled. `attention::observe` treats an `action`/`wait-reply` record
   whose token names a live item as "delivered": the item is rebuilt with
   `delivered = true`. A later `reopened` record still resets it, so ordering
   in the journal decides, as for `acknowledged`.

3. **Layout.** `dispatch.rs` is at 287 code lines (limit 300), so the wait
   request, waiter matching and `satisfy`/`expire` move into a new
   `wait_ops.rs` beside it (with `tests/wait_ops_test.rs`); `dispatch.rs`
   only routes `"wait"` there.

Reuse: `status::wanted_epic` matching rule, `Record::new`'s token field,
`attention::observe`'s replay path, the existing waiter list.

## Interfaces / Schema

- CLI: `toolu epic wait [--max-seconds N] [--epic EPIC]`.
- Socket request: `{"op":"wait","max_seconds":N,"epic":"EPIC"}`; `epic` optional.
- Socket error reply: `{"error":"unknown epic: EPIC"}`.
- Journal: `{"kind":"action","name":"wait-reply","key":K,"epic":E,"token":"a…","note":"judgment:<kind>"}`;
  timeouts keep `token: ""`.
- `attention::take(world: &mut World, scope: Option<&str>) -> Option<Attention>`;
  `status::wait_body(world, max_seconds, scope)`.
- `socket_loop::Waiter { deadline, reply, epic: Option<String> }`.

## Failure modes and edge cases

- Unknown or empty-registry `--epic`: error reply, exit non-zero through
  `failed("toolu epic wait", …)`; nothing is marked delivered.
- Engine not running: the CLI answers `{"state":"waiting"}` as today, with or
  without `--epic` (recovery.md relies on it).
- Two scoped waiters for different epics: each gets only its own epic's items
  plus engine-wide ones; an item no waiter's scope matches stays undelivered for
  a later matching waiter.
- Journal append fails after a reply: the reply was already sent; the error
  surfaces as today (best effort in `socket_loop`), and the item may replay
  after a restart.
- Journals written by older binaries: their `wait-reply` lines have no token, so
  those items replay once more after upgrade (today's behaviour), never worse.
- Older binaries reading new journals: they ignore an action record's token, so
  a downgrade behaves as today.
- Metrics: `wait-reply` still counts as one wake-up; a 0-second wait that hands
  out a judgment now also counts (it woke the orchestrator with a decision).

## Acceptance criteria

- **AC-1:** With items raised for epics `ea` and `eb` (registered), a wait
  request with `"epic":"eb"` returns `eb`'s item even though `ea`'s is older,
  and a later unscoped wait still returns `ea`'s item.
- **AC-2:** A scoped wait also returns an engine-wide item (`epic` empty).
- **AC-3:** A wait with an unknown epic selector replies
  `{"error":"unknown epic: nope"}` and delivers nothing; a trailing-number
  selector (`"9"` for `o-r-9`) resolves.
- **AC-4:** Two blocked waiters scoped to different epics are each satisfied
  with their own epic's item, regardless of arrival order.
- **AC-5:** After an item is handed out by `wait` and the engine is reopened
  over the same state root (`Engine::open` replaying the journal), a
  `--max-seconds 0` wait returns `{"state":"waiting"}`, not the item again; an
  item never handed out is still returned after the reopen.
- **AC-6:** A delivered item that is later reopened (`reraise`) is handed out
  again after a restart.
- **AC-7:** `toolu epic wait --help` lists `--epic`; `docs/cli/` and the
  `commands --json` snapshot include it; the epic-orchestrator skill's main loop
  passes `--epic <epic-key>`.
- **AC-8:** `toolu epic wait --epic 591 --max-seconds 0` sends a socket request
  carrying `"epic":"591"`; without `--epic` the request has no `epic` field.

## Acceptance evidence

| AC | Input | Expected | Check |
|----|-------|----------|-------|
| AC-1, AC-2, AC-4 | Real `Engine::open` on a temp root with two registered epics and items raised through `attention::raise` | Scoped replies as stated | `cargo test -p toolu-epic-orchestrator dispatch_wait` |
| AC-3 | Same engine, selectors `nope` and `9` | Error reply; resolved scope | same |
| AC-5, AC-6 | Engine opened, item delivered, engine dropped and reopened on the same root | `waiting`; undelivered and reopened items return | `cargo test -p toolu-epic-orchestrator wait_restart` |
| AC-5 (journal) | `wait-reply` row after a judgment reply | `token` equals the item token | `cargo test -p toolu-epic-orchestrator wait_journal` |
| AC-8 | The request `query::wait` builds from parsed `wait` arguments | `epic` present only when given | `cargo test -p toolu-epic-orchestrator query` |
| AC-7 | Built binary | `--epic` in help and generated docs | `cargo xtask docs-cli` check in the gate, `cargo test -p toolu-cli --test contract`, `cargo test -p toolu-cli --test epic_skill`, `cargo xtask check-markdown-cli` |

## Documentation impact

`docs/cli/` (generated), `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`
(main loop passes `--epic`), `plugins/epic-orchestrator/README.md` (wait
description), `AGENTS.md` key-files row only if it describes `wait` (it does
not today).

## Open Questions

None. Default scope and engine-wide items are decided above (non-goal 1,
architecture 1).

## Spec review

- Architecture: 🟡 should-fix: `dispatch.rs` near the 300-line limit. Fixed: `wait_ops.rs` (Architecture 3).
- Acceptance criteria: 🟡 should-fix: nothing proved the CLI forwards `--epic` (Jev AC-coverage noul 0.48). Fixed: AC-8.
- Interfaces: 🔵 consider: a trailing number matching several epics takes the first in key order (`world.epics` is a `BTreeMap`). Accepted; an exact key avoids it.
