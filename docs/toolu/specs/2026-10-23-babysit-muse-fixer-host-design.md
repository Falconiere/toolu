# pr-babysit: Muse as a fixer host — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Claude (epic worker, #604)   **Topic:** Add Muse Code to pr-babysit's fixer hosts so route-fix and dispatch-fix can use it

## Problem

pr-babysit's `Fixer` enum (`crates/pr-babysit/src/fixer/host.rs`) knows Claude,
Codex and Cursor only. A dispatch plan group with host `muse` is refused
(`plan group 1 names host 'muse'; use one of claude, codex, cursor`,
`dispatch/plan.rs:58`), `prBabysit.hosts: ["muse"]` is a config error, and
`toolu babysit route-fix --host muse` fails before routing because the controller
host cannot be parsed as a fixer. The epic engine now runs workers on Muse (#603,
PR #607), so a Muse worker's babysit round cannot hand fixes to a Muse fixer.

## Non-Goals

1. Hermes as a fixer host (the global `--host` accepts it; it stays a
   `config_invalid` for routing).
2. Resuming a Muse fixer conversation; fixers always start fresh.
3. Changing the Claude, Codex or Cursor launch contracts.
4. Muse-specific trust-prompt handling: `--yolo` and `--trust-workspace` start
   Muse in an untrusted worktree without a prompt (live probe below).

## Architecture

Add `Fixer::Muse` and mirror the epic engine's Muse launch contract
(`crates/epic-orchestrator/src/hosts.rs`, `route/table.rs`) so the two stay in
step:

- name `muse`, CLI `muse`, alias `muse` only (as `hosts::parse_host_kind`).
- approvals: unattended `--yolo`; attended `--approval-mode on-request --trust-workspace`.
- no name flag (Muse has none; Claude's `-n` stays Claude-only).
- model `--model <m>`; effort `--reasoning-effort <e>`.
- tier defaults: `muse-spark-1.3`/low, `muse-wonder-1.0`/medium,
  `muse-wonder-1.0`/high, `muse-wonder-full-1.0`/xhigh.

`Fixer::ALL` gains Muse; plan validation, config parsing, routing defaults and
`route-fix`'s PATH check already iterate it. Messages that list hosts become
`claude, codex, cursor, muse`.

**Session files.** A live probe (herdr 0.9.5, Muse Code 1.4.12, scratch repo,
`herdr agent start --kind muse -- --yolo …`) showed Muse writes nothing into the
worktree itself; its session log lives under `$XDG_DATA_HOME/muse/sessions`.
toolu's hooks running under Muse write project state to `<repo>/.toolu/tmp/muse/…`
(`docs/config.md`; seen as `.toolu/tmp/muse/telemetry` and `plan-ledger` in real
worktrees). `dispatch/worktree.rs` therefore also ignores untracked
`.toolu/tmp/` paths. Wider rules were rejected: untracked `.toolu/skills/…` is
real work (project skills), and `.agents/` and `.muse/` are tracked config that
Muse mounts read-only. Jev chose `.toolu/tmp/` only (0.92).

**Teardown.** `stop_agent` sends `esc` then `/exit`. With live background work
Muse asks "Exit anyway?" and waits; the epic engine sends `enter` after `/exit`
for Muse (`launch/lifecycle.rs` `request_exit`). `stop_agent` takes the fixer
host and does the same for Muse; an `agent_not_found` answer (an idle Muse has
already left) is ignored like any other failed key. Callers pass the host: the
launch failure path has it, and `watch.rs` reads the group's recorded `host`.

**Provider limits.** `dispatch/outcome.rs` `HOST_LIMIT` gains the epic engine's
Muse phrases `model limit reached` and `you've used your <plan> quota`, so a
limited Muse fixer ends `host_limited` and its host cools down instead of
`no_report`.

## Interfaces / Schema

```rust
pub(crate) enum Fixer { Claude, Codex, Cursor, Muse }
pub(crate) const ALL: [Fixer; 4];
// Fixer::Muse.args("pb-x-r1g1", Some("muse-wonder-1.0"), Some("high"), true)
//   == ["--yolo", "--model", "muse-wonder-1.0", "--reasoning-effort", "high"]
// Fixer::Muse.args(name, None, None, false)
//   == ["--approval-mode", "on-request", "--trust-workspace"]
pub(crate) fn stop_agent(env: &Env, name: &str, host: Option<Fixer>) -> bool;
```

`fixtures/pr-babysit/hosts/fixer-hosts.json`: `argv` grows from 24 to 32 cases
(Muse × model {none, `muse-wonder-1.0`} × effort {none, `high`} × attended);
`routing.muse` holds the four defaults; `aliases` gains `"Muse": "muse"`;
`hostLimit` matches `outcome.rs`.

Config: `prBabysit.hosts` may list `muse`; `prBabysit.routing.muse` overrides the
four tiers; `prefer.<tier>` may name `muse`.

## Failure modes and edge cases

- `muse` not on `PATH`: `route-fix` drops it (`CLI not on PATH: muse`), as for the
  other hosts; `dispatch-fix` ends the group `agent_start_failed` with
  `muse is not on PATH`.
- Unsafe Muse model or effort (e.g. `a b`): `config_invalid` naming the argument,
  before any side effect.
- Unknown host (e.g. `gemini`, `hermes`): `config_invalid`, message lists
  `claude, codex, cursor, muse`.
- Idle Muse on stop: `/exit` ends it; the `enter` lands at the shell or herdr
  answers `agent_not_found`; both ignored, the 30 s liveness wait decides.
- Muse with background work on stop: `enter` confirms "Exit anyway".
- Fixer worktree with only `.toolu/tmp/…` untracked files: clean; reuse and
  cleanup proceed. Untracked `.toolu/skills/…` or any other file: `worktree_dirty`.
- Muse prints a limit line without a report: `host_limited`.

## Acceptance criteria

- **AC-1:** For each of 8 Muse combinations (model none/`muse-wonder-1.0` ×
  effort none/`high` × unattended/attended), `Fixer::Muse.args` returns the
  recorded argv; the 24 existing combinations are unchanged.
- **AC-2:** `Fixer::parse("muse")` and `"Muse"` give Muse, `Fixer::Muse.cli()` is
  `muse`, and `Fixer::Muse.defaults()` equals the recorded `routing.muse` table.
- **AC-3:** A `dispatch-fix start --dry-run` plan whose group names host `muse`
  passes validation and records `herdr agent start <name> --kind muse --pane …
  -- --yolo --model … --reasoning-effort …`; an unknown host's message lists
  `claude, codex, cursor, muse`.
- **AC-4:** `route-fix --host muse` with `prBabysit.hosts: ["muse"]` and a `muse`
  executable on `PATH` routes `dispatch: herdr` groups with host `muse` and the
  tier's Muse model and effort; in a mixed pool `["claude", "muse"]` with
  `prefer.critical: ["muse"]` a critical item goes to `muse` and the rest to
  `claude`; with no `muse` on `PATH` and `hosts: ["muse"]` it falls back to
  inline noting `CLI not on PATH: muse`.
- **AC-5:** A fixer worktree whose only untracked files are under `.toolu/tmp/` is
  clean; one with an untracked `.toolu/skills/x/SKILL.md` is `worktree_dirty`.
- **AC-6:** `fixer_outcome` returns `host_limited` for Muse's `model limit reached`
  and `You've used your Pro quota` lines without a report.
- **AC-7:** Live: on herdr 0.9.5 with Muse Code 1.4.12, a `dispatch-fix start` /
  `wait` / `cleanup` round with a single Muse group applies a fix (the fixer's
  new commit on the slot branch, which the controller then pushes to the PR
  branch of a scratch repository's bare remote), `wait` reports the group
  `done` and stops the Muse agent, and cleanup leaves no Muse agent, no herdr
  workspace and no slot worktree or branch.

## Acceptance evidence

- AC-1, AC-2: `fixtures/pr-babysit/hosts/fixer-hosts.json` (32 argv cases,
  `routing.muse`, aliases) through
  `cargo test -p toolu-pr-babysit fixer::host`.
- AC-3: plan test with a `muse` group in `dispatch/tests/plan_test.rs` and the
  dry-run start in `dispatch/tests/start_test.rs` (or `tests/dispatch_test.rs`);
  `cargo test -p toolu-pr-babysit dispatch`.
- AC-4: `tests/route_test.rs` with a real executable `muse` file on a temporary
  `PATH`, and the not-on-PATH case; `cargo test -p toolu-pr-babysit route`.
- AC-5: `dispatch/tests/worktree_test.rs` on a real Git worktree with the two
  untracked layouts; `cargo test -p toolu-pr-babysit worktree`.
- AC-6: `dispatch/tests/outcome_test.rs` with both Muse lines;
  `cargo test -p toolu-pr-babysit outcome`.
- AC-7: manual live run recorded in the PR body and linked on #604: the round's
  JSON outputs, the fix commit on the scratch remote, and empty `herdr agent list`,
  `git worktree list` and `git branch --list 'pr-babysit/*'` afterwards.

## Documentation impact

- `plugins/pr-babysit/skills/babysit/references/helper.md`: route-fix synopsis and
  host/CLI lists name `muse`; session-artifact note names `.toolu/tmp/`.
- `plugins/pr-babysit/workflows/babysit.md`: controller host list names `muse`.
- `plugins/pr-babysit/README.md`: host CLI list names `muse`.
- Generated CLI reference: none — no flag or command changes.

## Open Questions

None blocking. The Muse model ids follow the epic engine's routing table (#603);
if those change, both tables change together (owner #591).

## Spec review

- Acceptance criteria: 🟡 should-fix (fixed): AC-4 only covered a Muse-only pool; the issue asks that route-fix can choose Muse within a pool. Added the mixed `["claude", "muse"]` pool with `prefer.critical`.
- Acceptance criteria: 🟡 should-fix (fixed): AC-7 said the fix commit is pushed by the round; fixers never push (fixer brief), the controller does. Reworded, and made `wait` stopping the Muse agent observable.
- Issue mapping: `Fixer::Muse` flags and defaults (AC-1, AC-2), worktree session files (AC-5), host messages (AC-3), plan group `muse` (AC-3), route-fix pool choice (AC-4), live herdr 0.9.5 round with cleanup and evidence on #604 (AC-7). Jev coverage judgment 0.71; manual mapping found no uncovered item.

**Status:** Approved
