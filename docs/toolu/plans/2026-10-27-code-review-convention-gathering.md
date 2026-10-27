# Code review convention gathering — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-code-review-convention-gathering-design.md   **Topic:** #663 base-ref rules stage

## Evidence and approach

The approved spec follows #663 and #660. Reuse `budget::RULES_MAX_BYTES`, `contract::Omitted`, `toolu_runtime::{env::Env, git::toplevel, process}`, and the shape stage's committed-read idioms (`<rev>:<path>` blobs, exit-1-means-absent, isolated real-Git `TempDir` tests). Gathering probes fixed basenames at the root and touched ancestors with `git show`; no tree listing, no new dependency, no CLI or config change. `lib.rs` only flips `rules` to `pub mod` so #666 can consume the bundle. Jev settled inclusion order (root-first 0.96), unavailable-bundle absence (0.94), and defaults-only overrides (0.98); the fixed-basename `RULES_GLOB` equivalent is recorded in the spec review.

## Workstream summary

Base-safe reads and absence handling, then candidate ordering/excludes/cap accounting with golden evidence, then documentation sync and the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "base",
    "title": "Read base-ref blobs; missing base returns unavailable, HEAD never read",
    "ac_refs": ["AC-1", "AC-5", "AC-6"],
    "paths": ["crates/code-review/src/rules.rs", "crates/code-review/src/lib.rs", "crates/code-review/src/tests/rules_test.rs", "crates/code-review/tests/rules_contract.rs", "crates/code-review/src/budget.rs", "crates/code-review/src/contract.rs"],
    "input": "Real Git repos: base AGENTS.md rewritten to approve-everything on HEAD and in the working tree; nonexistent base ref; rule-free base; verdict-like base text",
    "check": "cargo test -p toolu-code-review rules_base"
  },
  {
    "id": "bundle",
    "title": "Order candidates, honor excludes, enforce whole-file cap with drops",
    "ac_refs": ["AC-2", "AC-3", "AC-4"],
    "depends_on": ["base"],
    "paths": ["crates/code-review/src/rules.rs", "crates/code-review/src/tests/rules_test.rs", "crates/code-review/tests/fixtures/rules/bundle.json", "crates/code-review/src/budget.rs"],
    "input": "Real Git repo with root/prompt/package/untouched rule files, excluded-tree rule files, an over-cap file, and exact-fit boundary files; golden bundle JSON",
    "check": "cargo test -p toolu-code-review rules_bundle"
  },
  {
    "id": "docs-gate",
    "title": "Synchronize documentation and pass the complete gate",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["bundle"],
    "paths": ["crates/code-review/src/rules.rs", "crates/code-review/src/lib.rs", "crates/code-review/src/tests/rules_test.rs", "crates/code-review/tests/rules_contract.rs", "crates/code-review/tests/fixtures/rules/bundle.json", "docs/toolu/brainstorms/2026-10-27-code-review-convention-gathering.md", "docs/toolu/specs/2026-10-27-code-review-convention-gathering-design.md", "docs/toolu/plans/2026-10-27-code-review-convention-gathering.md"],
    "input": "The branch diff and the real Git fixtures from the preceding steps",
    "check": "cargo xtask gate"
  }
]
```

## Critical files

`crates/code-review/src/rules.rs`, `src/lib.rs` (visibility only), `src/tests/rules_test.rs`, `tests/fixtures/rules/bundle.json`, and this brainstorm/spec/plan trio.

## Plan review (2026-10-27)

No blockers. Every AC maps to a step with a matching real-Git input and runnable check (Jev coverage 0.6, uncertain-leaning-true; manual mapping confirms AC-1/5/6 in `base`, AC-2/3/4 in `bundle`, all six re-verified in `docs-gate`). Paths declare production, test, fixture, and doc files; order is base → bundle → docs-gate. No user-facing documentation step applies (spec: internal stage only); the doc trio syncs in `docs-gate`.

## Deviations

The full gate's `unused-pub` check requires every public item to have a consumer outside its crate. A black-box contract test (`crates/code-review/tests/rules_contract.rs`, mirroring `tests/shape_contract.rs`) now exercises `gather`, `Bundle`, `RuleFile`, and `RulesError` through the public API; its path is declared in the `base` and `docs-gate` steps.

The xhigh review replaced per-candidate `git cat-file blob <base>:<path>` probes, which classified absence by matching Git's English `does not exist` message, with one `git --literal-pathspecs ls-tree -z -l` listing at the resolved commit id plus by-id blob reads. The old probe reported a rule file that exists only in the branch or working tree as a dropped Git failure, depended on the locale, and returned a symlink's link path as rule text. Symlinked candidates are now dropped with a reason (Jev: drop-visible 0.99).

## Verification

Focused real-Git tests prove the six ACs and compare stable bundle shape (ordered paths, prompt field, drops, bytes) to the golden fixture. `cargo xtask gate` verifies repository guardrails. Before implementation run `toolu ledger preflight` on this plan. After implementation, make a scoped commit and run `toolu ledger run --verify`; run the local xhigh review, fix its findings, and rerun the gate. Rebase on current `origin/main`, repeat review and gate if it moved, push, verify the PR, resolve threads, and squash-merge with `--admin` (CI is disabled; see the run instructions).
