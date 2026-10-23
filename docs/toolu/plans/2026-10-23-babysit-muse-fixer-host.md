# pr-babysit: Muse as a fixer host — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-babysit-muse-fixer-host-design.md   **Topic:** add Fixer::Muse to route-fix and dispatch-fix

## Evidence and approach

Inspected `crates/pr-babysit/src/fixer/{host,config}.rs`, `route.rs`,
`dispatch/{plan,launch,herdr,watch,worktree,outcome}.rs`, their tests, the
shared `fixtures/pr-babysit/hosts/fixer-hosts.json`, and the epic engine's Muse
contract (`crates/epic-orchestrator/src/hosts.rs`, `route/table.rs`,
`launch/lifecycle.rs` `request_exit`). A live Muse 1.4.12 / herdr 0.9.5 probe in a
scratch worktree showed Muse writes no files into the worktree, and real
worktrees show toolu's Muse hooks writing `.toolu/tmp/muse/…`.

Adding `Fixer::Muse` makes the compiler list every `match`. Tests reuse the
existing real-Git sandbox (`dispatch::sandbox`) and executable herdr/CLI files on
a temporary `PATH`; no mocking crate.

## Workstream summary

host contract and fixture → plan validation and dry-run start → route-fix pool →
session files → teardown → limit lines → docs → live round → gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Fixer::Muse: name, cli, alias, approvals, effort flag, tier defaults, host messages; 32-case fixture",
    "check": "cargo test -p toolu-pr-babysit --lib fixer::",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["crates/pr-babysit/src/fixer/host.rs", "crates/pr-babysit/src/fixer/tests/host_test.rs", "crates/pr-babysit/src/fixer/config.rs", "crates/pr-babysit/src/fixer/tests/config_test.rs", "fixtures/pr-babysit/hosts/fixer-hosts.json"],
    "input": "fixture argv for muse × model {none, muse-wonder-1.0} × effort {none, high} × attended, the epic engine's Muse argv and route table; boundaries: model 'a b' refused, 'gemini' unknown with muse listed, prBabysit.routing.muse needs 4 tiers"
  },
  {
    "id": "s2",
    "title": "Plan group naming muse validates; dry-run start records herdr agent start --kind muse with Muse flags",
    "check": "cargo test -p toolu-pr-babysit --lib dispatch::",
    "depends_on": ["s1"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/pr-babysit/src/dispatch", "crates/pr-babysit/src/dispatch.rs", "crates/pr-babysit/src/fixer", "fixtures/pr-babysit"],
    "input": "route plan with one group {host: muse, model: muse-wonder-1.0, effort: high} on the real-Git sandbox; boundary: host 'hermes' fails config_invalid naming claude, codex, cursor, muse"
  },
  {
    "id": "s3",
    "title": "route-fix routes to muse alone and within a claude+muse pool; inline fallback when muse is not on PATH",
    "check": "cargo test -p toolu-pr-babysit --lib route",
    "depends_on": ["s1"],
    "ac_refs": ["AC-4"],
    "paths": ["crates/pr-babysit/src/route.rs", "crates/pr-babysit/src/route", "crates/pr-babysit/src/tests/route_test.rs", "crates/pr-babysit/src/fixer", "fixtures/pr-babysit"],
    "input": "fixtures/pr-babysit/items/review-items.json with heuristic scoring, an executable muse file on a temp PATH; boundaries: prefer.critical [muse] in a [claude, muse] pool, no muse on PATH"
  },
  {
    "id": "s4",
    "title": "Untracked .toolu/tmp/ files are session files; .toolu/skills/ is real work",
    "check": "cargo test -p toolu-pr-babysit --lib dispatch::worktree",
    "ac_refs": ["AC-5"],
    "paths": ["crates/pr-babysit/src/dispatch/worktree.rs", "crates/pr-babysit/src/dispatch/tests/worktree_test.rs"],
    "input": "real herdr-sandbox Git worktree with .toolu/tmp/muse/telemetry/x.json, then .toolu/skills/x/SKILL.md"
  },
  {
    "id": "s5",
    "title": "stop_agent sends enter after /exit for Muse only, tolerating agent_not_found",
    "check": "cargo test -p toolu-pr-babysit --lib dispatch::herdr",
    "depends_on": ["s1"],
    "ac_refs": ["AC-8"],
    "paths": ["crates/pr-babysit/src/dispatch/herdr.rs", "crates/pr-babysit/src/dispatch/tests/herdr_test.rs", "crates/pr-babysit/src/dispatch/watch.rs", "crates/pr-babysit/src/dispatch/launch.rs"],
    "input": "a herdr executable on a temp PATH that logs argv and lists the agent until /exit (claude) or enter (muse); boundary: enter answered agent_not_found"
  },
  {
    "id": "s6",
    "title": "HOST_LIMIT matches Muse's model-limit and plan-quota lines; fixture hostLimit in step",
    "check": "cargo test -p toolu-pr-babysit --lib dispatch::outcome",
    "ac_refs": ["AC-6"],
    "paths": ["crates/pr-babysit/src/dispatch/outcome.rs", "crates/pr-babysit/src/dispatch/tests/outcome_test.rs", "fixtures/pr-babysit/hosts/fixer-hosts.json"],
    "input": "Muse 1.4.12 lines 'model limit reached' and \"You've used your Pro quota\"; boundary: the quota warning 'At this pace your Pro quota runs out' stays no_report"
  },
  {
    "id": "s7",
    "title": "Docs: helper.md, babysit workflow, README name muse and .toolu/tmp/",
    "check": "cargo xtask check-markdown-cli && grep -q 'cursor-agent`, `muse`' plugins/pr-babysit/skills/babysit/references/helper.md && grep -q '.toolu/tmp/' plugins/pr-babysit/skills/babysit/references/helper.md && grep -q 'cursor` or `muse`' plugins/pr-babysit/workflows/babysit.md && grep -q '`cursor-agent`, `muse`' plugins/pr-babysit/README.md && ! grep -q 'claude|codex|cursor ' plugins/pr-babysit/skills/babysit/references/helper.md",
    "paths": ["plugins/pr-babysit/skills/babysit/references/helper.md", "plugins/pr-babysit/workflows/babysit.md", "plugins/pr-babysit/README.md", "docs/config.md"],
    "input": "the three agent-facing Markdown files; boundary: no host list still stops at cursor, and every toolu command in them parses against the real CLI tree"
  },
  {
    "id": "s8",
    "title": "Live dispatch-fix round with a Muse fixer on herdr 0.9.5 applies a fix and cleans up",
    "check": "grep -q 'kind muse' docs/toolu/evidence/babysit-muse-fixer-live.md && grep -q 'group 1: done' docs/toolu/evidence/babysit-muse-fixer-live.md && grep -q 'agents after cleanup: none' docs/toolu/evidence/babysit-muse-fixer-live.md && grep -q 'slot worktree after cleanup: none' docs/toolu/evidence/babysit-muse-fixer-live.md && grep -q 'slot branch after cleanup: none' docs/toolu/evidence/babysit-muse-fixer-live.md",
    "depends_on": ["s1", "s2", "s4", "s5"],
    "ac_refs": ["AC-7"],
    "paths": ["docs/toolu/evidence/babysit-muse-fixer-live.md"],
    "input": "scratch repo with a bare origin, PR branch with a typo, items file, route plan with one muse group; target/debug/toolu babysit dispatch-fix start/wait/cleanup; boundary: the fixer's commit reaches origin only through the controller's push, and cleanup refuses nothing because the slot branch is an ancestor of origin"
  },
  {
    "id": "s9",
    "title": "Full quality gate",
    "check": "cargo xtask gate --only gate-change --only fmt --only clippy --only guardrails --only layers --only reach --only deny --only machete --only unused-pub --only hooks --only workflows --only docs-cli --only cli-compat --only jscpd --only rust-quality --only docs --only context-budget --only final-removal --only gate-coverage --only packaging --only ci-paths --only bench --only pack && cargo test -p toolu-pr-babysit",
    "depends_on": ["s1", "s2", "s3", "s4", "s5", "s6", "s7"]
  }
]
```

## Critical files

- `crates/pr-babysit/src/fixer/host.rs`, `fixer/tests/host_test.rs`, `fixer/tests/config_test.rs`
- `crates/pr-babysit/src/dispatch/{plan,herdr,watch,launch,worktree,outcome}.rs` and their tests
- `crates/pr-babysit/src/tests/route_test.rs`
- `fixtures/pr-babysit/hosts/fixer-hosts.json`
- `plugins/pr-babysit/skills/babysit/references/helper.md`, `plugins/pr-babysit/workflows/babysit.md`, `plugins/pr-babysit/README.md`
- `docs/toolu/evidence/babysit-muse-fixer-live.md`

## Verification

Focused `cargo test -p toolu-pr-babysit` per step, red before green. The live
round (s8) runs the debug binary against real herdr 0.9.5 and Muse Code 1.4.12
and records the commands, outputs and the empty agent/worktree/branch listings.
`cargo xtask gate` before push; the Homebrew-toolu launcher-test deviation known
from #607 is recorded if it recurs. Docs are synchronized in s7.

## Deviations

- s7 also updates `docs/config.md`: its `prBabysit` table lists the fixer hosts and their routing defaults, which the spec's documentation impact missed.
- s9 runs `cargo xtask gate` without its `tests` and `coverage` steps, plus `cargo test -p toolu-pr-babysit`. On this Mac a Homebrew `toolu` on `PATH` fails the `toolu-cli --test launcher`, `toolu-jev` and `toolu-runtime` `startup::native` cases on a clean `origin/main` as well (the same deviation as #607). CI runs the full gate. The `toolu-cli` `contract`, `epic_markdown` and `epic_skill` tests pass with `TOOLU_RESOURCE_HOME` isolated.

## Plan review

- s2, s3: 🟡 should-fix (fixed): `dispatch::` and `route` filters run whole modules; paths widened to the module directories and `fixer/`.
- s7: 🟡 should-fix (fixed): `check-markdown-cli` alone does not prove the host lists changed; added literal checks and dropped its `ac_refs` (documentation, not AC proof).
- s8: 🟡 should-fix (fixed): `test -s` accepted any evidence; the check now requires the Muse launch, the done group and the three empty listings.
- AC coverage: AC-1, AC-2 s1; AC-3 s2; AC-4 s3; AC-5 s4; AC-6 s6; AC-7 s8; AC-8 s5. Jev rated the weakest step s7 (0.54), addressed above.
