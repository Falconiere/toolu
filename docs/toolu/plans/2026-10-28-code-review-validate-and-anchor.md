# Code-review validate and anchor (#668) — Plan

**Date:** 2026-10-28   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-28-code-review-validate-and-anchor-design.md   **Topic:** Ground every finding in the shaped diff: anchor, quote, dedupe, withdrawals, cross-round recap.

Plan review (2026-10-28): no blockers. AC-1…AC-9 each map to a step with a
runnable check and a named real input; dependencies are linear; the only
documentation surface (crate module docs) is step s4; delivery readiness is
the full gate plus the mandated local review cycle.

## Evidence and approach

Inspected: `crates/code-review/src/{lib,contract,model,memory,shape,prepass,validate}.rs`
(`validate.rs` is a one-line stub), `review.rs` + `review/mask.rs` (#666 judged
output and the secret `Mask`), `tests/records.rs` (fingerprint shape
`path \u{1f} category \u{1f} normalized claim`), epic #660 and issue #668
acceptance criteria including the 2026-10-27 review additions. Jev settled the
ambiguous defaults (normalized quotes 0.92, explicit withdrawal markers 0.87,
prose-suggestion heuristic 0.97, partial loss as unreviewed coverage 1.00).
New code lives in `crates/code-review/src/validate.rs` plus its sibling tests;
`review::mask` becomes `pub(crate)` for reuse. No CLI wiring (#669).

## Workstream summary

fingerprint/normalize + anchor/quote grounding → withdrawal, masking,
suggestion, suppression → dedupe + cross-round recap → docs note in `lib.rs`
→ full gate.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"validate: fingerprint/normalize_text, anchor index over shaped lines (new+old side), quote check, hallucination drop + unreviewed","check":"cargo test -p toolu-code-review validate::","ac_refs":["AC-1","AC-2","AC-3"],"paths":["crates/code-review/src/validate.rs","crates/code-review/src/tests/validate_test.rs"],"input":"constructed Shaped with added, removed, and deleted-file lines","model":"inherit"},
  {"id":"s2","title":"validate: withdrawal drop, secret masking via review Mask, prose-suggestion drop, unresolvable-redaction fail-closed","check":"cargo test -p toolu-code-review validate::","ac_refs":["AC-5","AC-6","AC-7"],"paths":["crates/code-review/src/validate.rs","crates/code-review/src/review/mask.rs","crates/code-review/src/tests/validate_test.rs"],"input":"no_defect/abstain claims, prose and code suggestions, redaction spans over a secret line","depends_on":["s1"],"model":"inherit"},
  {"id":"s3","title":"validate: dedupe (path,line,end_line,fingerprint) highest severity, deterministic tie-win and exemption, cross-round matching (prior_id then fingerprint through renames), severity drift, settled suppression, recap resolved/still_open/new","check":"cargo test -p toolu-code-review validate:: && cargo test -p toolu-code-review","ac_refs":["AC-4","AC-8","AC-9"],"paths":["crates/code-review/src/validate.rs","crates/code-review/src/tests/validate_test.rs"],"input":"duplicate findings across severities, priors open/resolved/dismissed on renamed and unreviewed paths","depends_on":["s2"],"model":"inherit"},
  {"id":"s4","title":"lib.rs: pub validate module, not-built note names only #669","check":"cargo test -p toolu-code-review && cargo test -p toolu-cli --test contract","ac_refs":["AC-1"],"paths":["crates/code-review/src/lib.rs"],"depends_on":["s3"],"model":"inherit"},
  {"id":"s5","title":"Full quality gate via epic job lease","check":"toolu epic job -- cargo xtask gate","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-8","AC-9"],"paths":["crates/code-review"],"depends_on":["s4"],"model":"inherit"}
]
```

## Deviations

- s4 also updated the two places pinning the old not-built text:
  `crates/cli/tests/code_review_cli.rs` (asserts `(#669)`) and
  `plugins/code-review/README.md` (report #669 only).

## Critical files

- `crates/code-review/src/validate.rs` (new implementation)
- `crates/code-review/src/tests/validate_test.rs` (new sibling tests)
- `crates/code-review/src/review.rs` (`mod mask` → `pub(crate) mod mask`)
- `crates/code-review/src/lib.rs` (`pub mod validate`, not-built note)

## Verification

Red → green per step with constructed real stage outputs (`Shaped`,
`ModelFinding`, `Memory`, `Redaction`) — the same values #666 produces and
#669 will consume; no mocks. Boundaries: hallucinated path/line, old-side
anchors, whitespace-only quote differences, all-findings-dropped (paths stay
unreviewed, never a pass), unresolvable redaction. `cargo xtask gate` last,
then the user-mandated local `/code-review xhigh --fix` cycle and PR.
