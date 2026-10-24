# Codex session capture across local-date folders — Plan

**Date:** 2026-10-24   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-24-codex-session-local-day-design.md   **Topic:** Falconiere/toolu#620

## Evidence and approach
- `crates/epic-orchestrator/src/launch/session.rs` `transcripts()` builds Codex roots as
  `day_dir(home, since)` and `day_dir(home, now)`; `day_dir` uses the UTC ISO date.
- `crates/epic-orchestrator/src/launch/muse_session.rs` `session_dirs()` already scans the
  UTC folders of `since − DAY_MS`, `since`, `since + DAY_MS` for the same reason.
- `metrics/tokens.rs` Codex reader recurses `sessions/` to depth 4 with no date folder
  assumption; unchanged (spec non-goal 3).
- Live probe (2026-10-24 08:39 local, 11:39 UTC): `TZ=Pacific/Kiritimati codex exec` filed
  `sessions/2026/10/25/rollout-2026-10-25T01-39-55-<id>.jsonl` while the UTC date was
  2026/10/24 — Codex uses its local date.
- Approach: add `DAY_MS` and `day_dirs(base, instants)` to `session.rs` (UTC folders one day
  either side of each instant); Codex uses `day_dirs(home, &[since, now])`, Muse uses
  `day_dirs(root, &[since])`.

## Workstream summary
Red tests for previous/next-day Codex folders → shared `day_dirs` helper wired into Codex
and Muse → live layout probe → full gate.

## Steps (machine-readable)
```json
[
  {"id": "s1", "title": "Codex capture finds a session in the previous- or next-day UTC folder and still refuses ambiguity", "check": "cargo test -p toolu-epic-orchestrator launch::session::tests", "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-7"], "paths": ["crates/epic-orchestrator/src/launch/session.rs", "crates/epic-orchestrator/src/launch/tests/session_test.rs"], "input": "real session_meta JSONL files in a temp CODEX_HOME: one under sessions/<UTC date of since - 1 day> expects Ok(Some(id)) (AC-1); one under <since + 1 day> expects Ok(Some(id)) (AC-2); a previous-day file plus a second fresh file in the since-day folder expects Ok(None) (AC-3); both single-file cases return Ok(None) before the fix (red); 257 rollouts aged one hour before since plus one fresh file expects Ok(Some(id)) (AC-7)"},
  {"id": "s2", "title": "Muse capture uses the shared day window unchanged", "check": "cargo test -p toolu-epic-orchestrator launch::muse_session::tests", "ac_refs": ["AC-4"], "depends_on": ["s1"], "paths": ["crates/epic-orchestrator/src/launch/session.rs", "crates/epic-orchestrator/src/launch/muse_session.rs", "crates/epic-orchestrator/src/launch/tests/muse_session_test.rs"], "input": "existing real Muse session.jsonl fixtures in a temp XDG_DATA_HOME; every existing case keeps its expected id or None after the refactor"},
  {"id": "s3", "title": "Real Codex files its rollout under its local-date folder when that differs from UTC", "check": "/tmp/probe620.sh", "ac_refs": ["AC-5"], "depends_on": ["s2"], "paths": ["crates/epic-orchestrator/src/launch/session.rs"], "input": "real codex-cli 0.164.0 exec in a temp directory with TZ=Etc/GMT+12 (before 12:00 UTC) or Pacific/Kiritimati; expects the local date to differ from the UTC date and a rollout naming that cwd under sessions/<local date>; exits nonzero otherwise"},
  {"id": "s4", "title": "Full quality gate", "check": "toolu epic job -- /tmp/gate620.sh", "depends_on": ["s3"], "paths": ["crates/epic-orchestrator", "docs/toolu"], "input": "the whole workspace: every gate step except tests and coverage, plus cargo test -p toolu-epic-orchestrator"}
]
```

## Critical files
- `crates/epic-orchestrator/src/launch/session.rs`
- `crates/epic-orchestrator/src/launch/muse_session.rs`
- `crates/epic-orchestrator/src/launch/tests/session_test.rs`

## Verification
- Red first: the new previous-day and next-day tests return `Ok(None)` before the helper is
  wired in; green after.
- Boundary/failure: ambiguity across folders → `Ok(None)`; older or foreign sessions stay
  rejected by the existing tests; month/year rollover is handled by `day_dir` on full ISO
  dates.
- Live: `/tmp/probe620.sh` shows the real Codex layout; AC-6 is verified post-merge by the epic #591
  run.
- AC-6 (late-day engine launch on herdr 0.9.5) has no ledger step: it is post-merge and
  engine-driven, owned by the epic #591 run per the spec's Open Questions.
- Documentation: none user-facing (spec); doc comments in `session.rs`/`muse_session.rs`
  updated in s1/s2.
- Delivery: scoped commit, `toolu ledger run <plan> --verify`, toolu-review, verdict ready,
  push, PR with `Closes Falconiere/toolu#620` / `Part of Falconiere/toolu#591`, then the
  babysit handoff (`toolu epic report babysit --pr N`).

## Deviations

- s4: this machine's installed Homebrew toolu and live epic engine fail unrelated `tests`
  and `coverage` cases (launcher, muse_lifecycle, npm_wrapper, contract schema; verified on
  clean `origin/main` by earlier workers). s4 runs every other gate step plus the changed
  package's tests through `/tmp/gate620.sh`; CI runs the full gate.
- s1: local review found the wider window makes the 256-file cap likelier to trip on old
  rollouts; `jsonl_files` now counts only files fresh since `since` (spec AC-7).

## Plan review

**Status:** Approved

- s3: 🟡 should-fix (fixed): the probe first asserted only the local folder; it now also requires the local date to differ from UTC, picking UTC-12 or UTC+14 so it is runnable at any hour.
- AC coverage: AC-1..AC-5 mapped; AC-6 justified as a post-merge, engine-driven check owned by the epic #591 run.
- Order s1 → s2 → s3 → s4 is acyclic; s2 depends on the helper s1 adds.
