# Code-review validate and anchor — Design

**Date:** 2026-10-28   **Status:** Approved   **Author:** epic worker (muse)   **Topic:** Ground every model finding in the shaped diff before anything is reported (#668, part of epic #660)

Spec review (2026-10-28): no blockers. Two should-fix items were corrected in
place — the suggestion step no longer implies a finding can survive a range
that failed anchoring, and an unknown echoed `prior_id` explicitly falls
through to fingerprint matching. Every AC names a fixture and a runnable
check; scope stays inside #668 (no run wiring, no persistence).

## Problem

The review stage (#666) returns judged model findings that can cite lines that
do not exist, quote text the diff does not contain, repeat each other, or
withdraw themselves. Reporting them ungrounded makes agents pay for invented
defects and can turn lost coverage into a clean pass. This stage validates and
anchors every finding against the shaped diff (#662) before #669 reports it.

## Non-Goals

1. Wiring `toolu code-review run` end to end, the verdict report, or state
   writes — #669 owns those; `run` stays fail-closed as `unconfigured`.
2. Re-judging findings with Jev or dismissing them — #666 owns the judge and
   verified dismissals.
3. Persisting review memory — #669 owns it; this stage only computes the recap
   and the line-independent fingerprints #669 stores.

## Architecture

A new public `validate` module in `crates/code-review`, beside the existing
stage modules (`shape`, `prepass`, `review`). One pure function:

```rust
pub fn validate(
  shaped: &Shaped,
  model: &[ModelFinding],
  deterministic: &[Finding],
  memory: &Memory,
  redactions: &[Redaction],
  unreviewed: &[Omitted],
) -> Validated
```

`unreviewed` is the review stage's lost coverage (failed or oversized
review calls, #666's `Reviewed.unreviewed`): those paths were not
re-examined, so their open priors cannot resolve.

No I/O, no network, no verdict. `Validated` carries the anchored findings,
the added `unreviewed` coverage, a round `Recap`, and the grounding-drop
count. The module reuses `review`'s secret `Mask` (made `pub(crate)`) built
from #664's `Redaction` spans over the shaped lines, so a secret is replaced
by `[REDACTED]` in every outbound quote, claim, and suggestion. Deterministic
findings pass through untouched: quote checks and Jev never drop
`source: ast-grep | secrets` findings.

Decisions from Jev judgments over the alternatives (2026-10-28): quote
comparison is whitespace-normalized, not exact and not substring (0.92);
withdrawals drop only on explicit markers (0.87); prose suggestions drop on a
prose heuristic, keeping the finding (0.97); partial loss is recorded as
`unreviewed` coverage, never as a new error kind (1.00).

## Interfaces / Schema

```rust
pub struct Validated {
  pub findings: Vec<Finding>,     // anchored, deduped, worst-first-ready
  pub unreviewed: Vec<Omitted>,   // paths whose coverage was lost, with reason
  pub recap: Recap,               // resolved / still_open / new finding ids
  pub dropped: usize,             // grounding drops only (see below)
}
pub struct Recap {
  pub resolved: Vec<String>,
  pub still_open: Vec<String>,
  pub new: Vec<String>,
  pub severity_changes: Vec<SeverityChange>, // {id, from, to}
}
pub fn fingerprint(path: &str, category: &str, claim: &str) -> String
pub fn normalize_text(text: &str) -> String
```

- Fingerprint: `path \u{1f} category \u{1f} normalized claim`, matching the
  shape #661's records already serialize. Normalization lowercases, collapses
  every whitespace run to one space, and trims; it is line-independent.
- Finding ids: a finding matched to an open or resolved prior keeps that
  prior's id; a new finding gets `cr-<16 hex>` from an FNV-1a hash of its
  fingerprint plus its cited range and side, so identical claims at
  different lines are different findings. A blocker re-raised against a
  dismissed prior (not suppressed, below) reopens under that fresh id
  instead of inheriting the settled one.
- Anchor lookup: a `HashMap<(path, side, line), &str>` built once from every
  `ShapedFile.lines`; removals and deleted files anchor on `Side::Old`, so a
  finding about a removed line is not dropped as hallucinated.

Processing order per model finding:

1. **Withdrawal drop.** Category is `no_defect`/`no-defect`, or the normalized
   claim is or starts with `no defect`, `no defect, abstain`, or `abstain`.
   Dropped silently: not in `dropped`, no count, no verdict weight.
2. **Anchor.** Path must be a shaped file and every line in `line..=end_line`
   must be a changed line on the cited `side`. Otherwise drop, count it, and
   add the claimed path to `unreviewed` naming the lost line range.
3. **Quote.** The whitespace-normalized quote must equal the normalized cited
   lines joined by `\n`. A mismatch drops the finding, counts it, and marks
   that coverage `unreviewed` — never a clean pass.
4. **Mask, then suggestion.** Quote, claim, and suggestion are masked; the
   reported quote is the masked cited diff text itself (the model only ever
   saw the masked patch, so quotes are compared masked, case-sensitively,
   with whitespace runs collapsed). On a surviving finding, a suggestion is
   literal replacement code or it is dropped (the finding stays): prose has
   no code punctuation (`(){}[];` etc.) and at least three words, or starts
   with a prose word (`consider`, `you`, `this`, `prefer`, `try`, or `use`
   without `::`) even when it carries inline code. A finding whose cited
   range did not survive anchoring is already dropped at step 2, suggestion
   and all — deterministic findings carry no model suggestion to check.
   `redaction_spans` are carried only for single-line findings: spans index
   one line in the contract, and a multi-line quote is already masked.
5. **Settled suppression.** An echoed id present in `memory.dismissals`
   suppresses at any severity (the echo is the exact match). Matched
   against a `Dismissed` prior (step order below): non-blocking findings
   are suppressed on any match; a blocking finding is suppressed only on
   an exact `prior_id` echo. Suppressed findings are not counted and never
   resent.
6. **Dedupe.** By `(path, line, end_line, text-fingerprint)` over model and
   deterministic findings together, before any send (#669). Highest severity
   survives; ties keep the deterministic finding, then the first seen.

Cross-round matching, in order: echoed `prior_id` (exact id of a known
prior; an unknown id falls through to fingerprint matching), then fingerprint, with #662's rename map applied to the prior's path
(`old_path` → `path`) so a `git mv` keeps the finding open. Severity drift on
a matched open prior keeps the highest severity and records the change in
`recap.severity_changes`.

Recap over `Open` priors only: matched by a survivor → `still_open` (prior
id) — including when a deterministic finding wins the dedupe tie over the
model duplicate that re-raised it, whose prior match transfers to the
winner; unmatched but its (rename-mapped) path is in
`shaped.coverage.reviewed` and in neither this stage's added `unreviewed`
nor the review stage's `unreviewed` input → `resolved` (it was re-examined
and not re-raised); otherwise → `still_open` (unreexamined findings are never
marked resolved). Survivors with no prior match → `new`.

## Failure modes and edge cases

- **No judgment at all** (provider error, unusable structured output) never
  reaches this stage: `model::parse` / `review::RunError` already turn it
  into an `error` verdict. Partial loss here is `unreviewed` coverage, which
  the computed verdict treats as blocked, never approved.
- **Every model finding dropped:** findings are empty and the lost paths are
  `unreviewed`, so #669 cannot report a clean pass.
- **Unresolvable secret redaction:** if a redaction span cannot be located in
  the shaped lines, findings for that path are dropped as unreviewed rather
  than risk sending an unmasked quote.
- **Model-supplied verdict:** none is read anywhere; `Report::verdict` is
  already computed from validated findings only.
- Empty model input, empty priors, and empty deterministic input all return
  empty results with an empty recap (open priors still resolve per the
  re-examined rule).

## Acceptance criteria

- **AC-1:** A model finding citing a line that is not a changed line on its
  side (or a path not shaped) is dropped and counted, and its path appears in
  `unreviewed` naming the lost range.
- **AC-2:** A finding whose quote does not match the cited changed lines
  (whitespace-normalized) is dropped; its coverage is `unreviewed`, never a
  pass. A quote differing only in whitespace runs survives.
- **AC-3:** A finding about a removed line or a deleted file anchors on
  `Side::Old` and survives when its quote matches the old text.
- **AC-4:** Duplicate findings (same path, line, end_line, text-fingerprint)
  collapse to one survivor with the highest severity; a deterministic
  duplicate wins ties over a model duplicate.
- **AC-5:** Withdrawn findings (`no_defect` category or an explicit
  `no defect`/`abstain` claim) are absent from findings, from `dropped`, and
  from every count; a real defect whose claim merely mentions "no defect" in
  passing survives.
- **AC-6:** A prose suggestion is dropped while its finding survives; a
  literal code suggestion survives.
- **AC-7:** Deterministic `ast-grep`/`secrets` findings pass through even
  when their quote cannot be re-verified against the shaped lines, and every
  surviving model quote/claim/suggestion has secret spans masked.
- **AC-8:** Cross-round match by `prior_id` first, then fingerprint through
  the rename map: a renamed file's re-raised finding keeps the prior id and
  is `still_open`, not resolved-plus-new; severity drift keeps the highest
  severity and is recorded.
- **AC-9:** An open prior on a reviewed path that is not re-raised is
  `resolved`; an open prior on a path not re-examined this round stays
  `still_open`; a dismissed prior suppresses a re-raised non-blocking
  finding but a blocking one only on an exact `prior_id` echo.

## Acceptance evidence

All checks run in the crate's sibling unit tests (`src/tests/validate_test.rs`,
real constructed `Shaped`/`ModelFinding`/`Memory` values, no mocks) plus the
crate test suite:

| AC | Fixture | Expected | Check |
| --- | --- | --- | --- |
| AC-1 | finding at line 99 of a 2-line change | dropped=1, path unreviewed | `cargo test -p toolu-code-review validate` |
| AC-2 | quote with wrong text; quote with extra spaces | first dropped+unreviewed, second survives | same |
| AC-3 | old-side finding on a removed line | survives with old quote | same |
| AC-4 | same finding at low and blocker; LLM copy of a secrets finding | one blocker survivor; secrets survivor | same |
| AC-5 | `no_defect`, "no defect, abstain", defect mentioning no defect | first two absent and uncounted, third survives | same |
| AC-6 | prose suggestion; `let x = 1;` suggestion | suggestion `None`; suggestion kept | same |
| AC-7 | secrets finding with unverifiable quote; model quote containing a secret | passes through; quote masked | same |
| AC-8 | prior on renamed path re-raised at a new line with higher severity | prior id, still_open, drift recorded | same |
| AC-9 | priors on reviewed/unreviewed paths; dismissed blocking/non-blocking re-raises | resolved/still_open; suppression per exact-only rule | same |

Full gate: `cargo xtask gate` (via `toolu epic job --`) before delivery.

## Documentation impact

`crates/code-review/src/lib.rs` module docs: `validate` becomes public and
the not-built note names only #669. No CLI, skill, or user docs change; the
report contract (`contract.rs`, `commands.json`) is untouched.

## Open Questions

None blocking. The two Jev-settled defaults (whitespace-normalized quotes,
explicit-marker withdrawals) and the re-examined = reviewed-and-not-lost
definition are recorded above for #669 to consume as-is. The local
`/code-review xhigh --fix` pass (2026-10-28) added the masked quote
comparison, the failed-review `unreviewed` input, dismissal-by-id,
range-scoped fresh ids, prose-starter suggestions, dedupe prior transfer,
and single-line spans; each has a regression test that fails on the
pre-review code.
