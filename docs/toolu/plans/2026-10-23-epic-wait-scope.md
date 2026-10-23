# Scoped and restart-safe `toolu epic wait` — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-epic-wait-scope-design.md   **Topic:** Falconiere/toolu#588

## Evidence and approach

Inspected: `query.rs::wait` (CLI, sends only `max_seconds`), `dispatch.rs::reply_wait`/`satisfy`/`expire`
(one waiter list, first waiter takes the oldest item), `status.rs::wait_body`,
`attention.rs::take` (in-memory `delivered`) and `attention::observe` (journal
rebuild, `judgment` records only), `wait_journal.rs::replied` (no token),
`recover.rs::observe` (calls `attention::observe` for every record),
`metrics/counts.rs` (counts `wait-reply` by note only, so a token is safe).
Tests reuse the real `Engine::open` on a temp root (`tests/dispatch_test.rs`),
which replays the journal, so a drop and reopen is a real restart.

Approach as the spec: `wait-reply` carries the item token and the rebuild marks
it delivered; `wait --epic` resolves against `world.epics` and scopes
`attention::take`; waiter handling moves to `wait_ops.rs` for the size limit.

## Workstream summary

restart-safe delivery → scoped engine wait → CLI flag → docs and skill → full gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "wait-reply journals the item token; the rebuild marks that item delivered; 0-second judgment replies are journaled", "check": "cargo test -p toolu-epic-orchestrator -- wait_journal wait_ops attention recover dispatch", "ac_refs": ["AC-5", "AC-6"], "paths": ["crates/epic-orchestrator/src/wait_journal.rs", "crates/epic-orchestrator/src/attention.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/wait_ops.rs", "crates/epic-orchestrator/src/socket_loop.rs", "crates/epic-orchestrator/src/status.rs", "crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/tests/wait_journal_test.rs", "crates/epic-orchestrator/src/tests/attention_test.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs", "crates/epic-orchestrator/src/tests/wait_ops_test.rs", "crates/epic-orchestrator/src/tests/socket_loop_test.rs", "crates/epic-orchestrator/src/tests/status_test.rs", "crates/epic-orchestrator/src/tests/limit_test.rs", "crates/epic-orchestrator/src/tests/logic_test.rs", "crates/epic-orchestrator/src/tests/schedule_test.rs"], "input": "real Engine::open on a temp root: two items raised; wait hands out the first (journal wait-reply row carries its token), engine dropped and reopened, then a 0-second wait returns the second item and a further one returns waiting (AC-5); a delivered item reraised before the drop is handed out again after the reopen (AC-6)", "model": "inherit"},
  {"id": "s2", "title": "wait_ops.rs: scoped waiters, epic selector resolution, unknown-epic error, per-waiter satisfy", "check": "cargo test -p toolu-epic-orchestrator -- wait_ops dispatch socket status limit logic schedule", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/wait_journal.rs", "crates/epic-orchestrator/src/attention.rs", "crates/epic-orchestrator/src/dispatch.rs", "crates/epic-orchestrator/src/wait_ops.rs", "crates/epic-orchestrator/src/socket_loop.rs", "crates/epic-orchestrator/src/status.rs", "crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/tests/wait_journal_test.rs", "crates/epic-orchestrator/src/tests/attention_test.rs", "crates/epic-orchestrator/src/tests/dispatch_test.rs", "crates/epic-orchestrator/src/tests/wait_ops_test.rs", "crates/epic-orchestrator/src/tests/socket_loop_test.rs", "crates/epic-orchestrator/src/tests/status_test.rs", "crates/epic-orchestrator/src/tests/limit_test.rs", "crates/epic-orchestrator/src/tests/logic_test.rs", "crates/epic-orchestrator/src/tests/schedule_test.rs"], "input": "real engine with epics o-r-1 and o-r-9 registered in world.epics: o-r-1 item raised first, then o-r-9 item; wait epic o-r-9 returns the o-r-9 item and an unscoped wait then returns the o-r-1 item (AC-1); a keyless item reaches a wait scoped to o-r-9 (AC-2); selector nope replies error unknown epic: nope with nothing delivered, selector 9 resolves to o-r-9 (AC-3); two blocked waiters, o-r-9 first then o-r-1, satisfied after an o-r-1 item and an o-r-9 item are raised, each receive their own epic item (AC-4)", "model": "inherit"},
  {"id": "s3", "title": "toolu epic wait --epic forwards the selector", "check": "cargo test -p toolu-epic-orchestrator -- query", "ac_refs": ["AC-8"], "depends_on": ["s2"], "paths": ["crates/epic-orchestrator/src/query.rs", "crates/epic-orchestrator/src/verbs.rs", "crates/epic-orchestrator/src/tests/query_test.rs"], "input": "argv wait --epic 591 --max-seconds 0 and argv wait --max-seconds 0 parsed by the real clap wait command; the built request has epic 591 only in the first", "model": "inherit"},
  {"id": "s4", "title": "Docs: generated CLI reference, commands snapshot, skill main loop and README pass --epic", "check": "cargo xtask docs-cli && cargo test -p toolu-cli --test contract --test epic_skill && cargo xtask check-markdown-cli", "ac_refs": ["AC-7"], "depends_on": ["s3"], "paths": ["docs/cli", "crates/cli/tests/fixtures/commands_json.snap", "crates/cli/tests/epic_skill.rs", "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md", "plugins/epic-orchestrator/README.md"], "input": "the built toolu binary: docs/cli/epic/wait page and commands_json.snap list --epic; SKILL.md main loop runs toolu --json epic wait --epic <epic-key>, asserted by epic_skill", "model": "inherit"},
  {"id": "s5", "title": "Full quality gate", "check": "cargo xtask gate --base origin/main", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8"], "depends_on": ["s4"], "paths": ["crates", "docs", "plugins", "tooling"], "input": "the whole branch", "model": "inherit"}
]
```

## Critical files

`crates/epic-orchestrator/src/{wait_journal,attention,dispatch,wait_ops,socket_loop,status,query,verbs,lib}.rs`,
their `tests/*_test.rs`, `docs/cli/`, `crates/cli/tests/fixtures/commands_json.snap`,
`crates/cli/tests/epic_skill.rs`, `plugins/epic-orchestrator/{README.md,skills/epic-orchestrator/SKILL.md}`.

## Verification

Red first: s1's restart test fails on main (the item replays) and s2's scoped
test fails (the older other-epic item comes back). Then green per step, and
`cargo xtask gate --base origin/main`. Boundaries: unknown selector, trailing
number, keyless item, reopened item, 0-second wait. Docs: skill, README,
generated `docs/cli/`.

Delivery: commit per green step (`wip:` subjects allowed, squash-merged), push
`feat/588-epic-wait-can-t-be`, then `toolu ledger run <plan> --verify`,
`toolu-review:review`, `toolu ledger verdict status` = `overall: ready`, PR
with `Closes Falconiere/toolu#588` / `Part of Falconiere/toolu#591`.

## Deviations

- s1/s2: the restart test lives in `tests/wait_ops_test.rs` (the module that
  hands items out), and `status::wait_body` now takes the scope and returns the
  item token, so its existing callers in `status_test`, `limit_test`,
  `logic_test` and `schedule_test` changed signature only. Paths and checks
  updated to match.

## Plan review

- s4: 🟡 should-fix: snapshot path was `crates/cli/tests/snapshots`, which does not exist. Fixed: `crates/cli/tests/fixtures/commands_json.snap`.
- Steps: 🟡 should-fix: inputs did not name each AC's boundary (reopened item, arrival order, unknown and trailing-number selectors). Fixed; Jev per-AC coverage 0.94–0.95 for AC-1..AC-8.
- Verification: 🔵 consider: delivery order was implicit. Added.
