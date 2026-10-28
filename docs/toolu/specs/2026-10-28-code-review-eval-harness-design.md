# Code-review paired-comparison eval harness — Design

**Date:** 2026-10-28   **Status:** Draft   **Author:** Cursor epic worker   **Topic:** #670 expectation-based eval

## Problem

The native code-review pipeline can review a diff, but nothing yet proves review
quality against known defects and known false positives. Token count and finding
count are the wrong bar. Agents and owners need a rerunnable paired comparison
(baseline without Jev vs Jev-enhanced) over pinned diffs with explicit
expectations, offline self-tests in the gate, and paid live/replay runs only on
an explicit command.

## Non-Goals

1. Changing the production `run` path beyond consuming the #666 `skip_judge` /
   `recorder` hooks already shipped.
2. Putting live OpenRouter or Jev calls inside `cargo xtask gate` or ordinary
   `cargo test`.
3. GitHub comment posting, auto-fix, MCP, or a new JS runtime.
4. A spend ceiling; cost is reported, never used to cut a run.
5. Replacing the push-review gate; the harness only asserts the gate contract
   through expectations over harness outputs.

## Architecture

Add an `eval` module in `crates/code-review` that:

1. Loads a case set from `fixtures/code-review-eval/` (pinned diff or mini-repo
   revision, expectations JSON, optional committed replay blobs).
2. Runs two arms over the same shaped input via existing `review::Options`:
   baseline (`skip_judge: true`) and enhanced (`skip_judge: false`).
3. Matches findings to expectations by exact `path` and case-insensitive
   substring over `claim` only. Unmatched findings are reported as
   `unclassified` for human adjudication; unmatched expectations are misses.
4. Writes a machine-readable report (revisions, coverage, findings, calls with
   live/replayed, usage, cost, runtime, tokens-per-finding, budget adherence,
   per-effort calibration when requested).
5. Uses a replay store keyed by `stage` + SHA-256 of the exact masked request
   body from `Record`. A hit returns the stored response and charges nothing;
   every call in the report states `live` or `replayed`.

`toolu code-review eval` is the explicit paid/replay entry. Offline
`cargo test` covers expectation load, matcher, report shaping, gate-contract
assertions, and memory expectation cases over synthetic findings and
committed fixture files — not a live network. A missing key on a live-required
run fails closed as blocked/unverified, never a green skip.

Decisive trade-off: keep the gate networkless by testing the matcher/report in
CI, and reserve full pipeline + replay/live for the explicit CLI (Jev
`matcher_only` 0.91). Reuse `Options.skip_judge` / `recorder` rather than a
parallel review path.

## Interfaces / Schema

### CLI

```text
toolu code-review eval --cases <DIR> [--arm both|baseline|enhanced]
  [--effort auto|small|medium|high|xhigh|none]... [--live] [--replay-dir <DIR>]
  [--report <PATH>]
```

- Default: both arms, replay-only (`--live` required for network).
- `--effort none` is the known-weak reasoning-off baseline for calibration.
- Exit: 0 when every required expectation is satisfied under the selected arms;
  2 when expectations miss or gate-contract checks fail; 69 when live was
  required and the key/opt-in/provider is unavailable; 64 on usage errors.

### Expectations file (`expectations.json`)

```json
{
  "version": 1,
  "cases": [
    {
      "id": "slice-underflow",
      "class": "supported",
      "diff": "diffs/slice-underflow.diff",
      "memory": null,
      "expectations": [
        {
          "path": "src/lib.rs",
          "claim": "slice underflows",
          "kind": "defect"
        },
        {
          "path": "src/lib.rs",
          "claim": "prefer rewrite",
          "kind": "false_positive"
        }
      ],
      "gate": {
        "blocking_severities_drive_count": true,
        "advisory_never_blocks": true,
        "unreviewed_never_writes_pass": true
      }
    }
  ]
}
```

`kind` is `defect` | `false_positive`. `class` ports upstream fixture classes:
`supported`, `contradicted`, `insufficient`, `adversarial`, `cross_file`,
`prompt_false_positive`, plus memory classes `memory_drift`,
`memory_unreexamined`, `memory_missing`.

### Matcher

For each expectation, a finding matches when `finding.path == expectation.path`
and `expectation.claim` is a case-insensitive substring of `finding.claim`.
First unused matching finding wins. Leftover findings → `unclassified`.
Missing defect expectations → miss. A `false_positive` expectation that matches
is a flagged known FP (counted, not a miss); absence of that FP is fine.

### Replay store

Directory of files named `<stage>-<sha256>.json` containing the recorded
response body. The recorder writes on `--live`; reads on default replay mode.
Request hash input is the exact `Record.request` string.

### Report (JSON)

Fields: `schema`, `revisions`, `arms[]` each with `effort`, `findings`,
`coverage`, `calls[{stage,model,provider,quantization,live_or_replayed,tokens,cost,millis}]`,
`usage`, `cost_usd`, `runtime_ms`, `tokens_per_finding`, `budget`,
`matches[{expectation_id,status}]`, `unclassified[]`, `calibration` (optional).

Quality summary prefers defect-catch rate and false-positive rate; never ranks
arms by fewer findings or fewer tokens alone. Document that bar in
`docs/cli/code-review.md` and the plugin README.

### Starting fixture set

Commit at least: the two epic probe diffs (slice underflow; inverted ownership),
one cross-file defect case, one prompt false-positive case, one contradicted /
insufficient class stub if expressible offline, and memory cases for drifted
line (stays open), unreexamined (never resolved), and missing history (full
review). Gate-contract expectations assert blocking severities drive
`findings_count`, advisory never blocks, and unreviewed coverage never writes
passing state (checked against harness-derived counts / synthetic state inputs
offline).

## Failure modes and edge cases

| Input | Behavior |
| --- | --- |
| Missing expectations file or unknown version | Exit 64 with a clear error; no network. |
| `--live` without `OPENROUTER_API_KEY` or repo opt-in | Exit 69 blocked/unverified; not a skip. |
| Replay miss in default (non-live) mode | Exit 69 unverified for that case; report names the missing key. |
| Empty findings vs defect expectations | Misses reported; exit 2. |
| Extra findings | Listed `unclassified`; do not fail solely for extras unless a case marks strict. |
| Claim phrasing drift | Distinctive expectation substrings; miss → human adjudication via unclassified/miss. |
| Provider outside pinned quantization set on live | Flag in report; does not rewrite history. |
| Gate path during `cargo test` | Never opens sockets; replay/live code paths are not invoked by default tests. |

## Acceptance criteria

- **AC-1:** Given a committed expectations file and pinned diff, the harness
  matches findings by exact path and case-insensitive claim substring; unmatched
  findings are `unclassified`.
- **AC-2:** The golden offline set includes expected defects, known false
  positives, a cross-file defect, and at least one unmatched/unclassified path
  exercised by a self-test.
- **AC-3:** The eval report includes revisions, coverage, findings, model calls
  (each `live` or `replayed`), usage, cost, runtime, and tokens-per-finding;
  replayed baseline calls add no second charge.
- **AC-4:** `toolu code-review eval` without `--live` never contacts the
  network; with `--live` and a missing key it exits 69 unverified, never green.
- **AC-5:** Memory expectations: drifted line stays open; unreexamined findings
  stay open; missing history triggers full review — proven with fixture memory
  inputs offline.
- **AC-6:** Gate-contract expectations: blocking severities drive blocking
  count; advisory never blocks; unreviewed coverage never writes passing state.
- **AC-7:** Quality bar documentation states that fewer findings or fewer tokens
  alone is not better; judgments use expectation outcomes.
- **AC-8:** Harness language is native Rust (crate module + black-box tests +
  CLI); no new JS runtime; `cargo test -p toolu-code-review` / focused tests
  stay green under the gate.
- **AC-9:** Effort calibration mode can run each case at fixed levels plus
  `auto` and `none`, reporting defects caught, FPs, tokens, cost, and how often
  `auto` chose below the lowest level that caught all defects — available via
  the explicit eval command (live or replay), not required inside the gate.

## Acceptance evidence

| AC | Real input | Observable result | Check |
| --- | --- | --- | --- |
| AC-1 | Fixture findings JSON + expectations | Match/miss/unclassified statuses | `cargo test -p toolu-code-review` matcher tests |
| AC-2 | `fixtures/code-review-eval/` golden set | Defects caught, FPs flagged, unclassified present | black-box / unit over fixtures |
| AC-3 | Synthetic call ledger with replay hits | Report fields; cost unchanged on replay | report unit tests |
| AC-4 | CLI invocation without `--live`; live without key | No network / exit 69 | black-box CLI test |
| AC-5 | Memory fixture cases | Status transitions as specified | memory expectation tests |
| AC-6 | Synthetic reports with mixed severities / unreviewed | Gate assertions pass/fail correctly | gate-contract tests |
| AC-7 | Docs + README | Explicit quality-bar wording | file content + markdown CLI check |
| AC-8 | Crate tests under gate | Green, no JS added | `cargo test -p toolu-code-review`; gate |
| AC-9 | Eval CLI help + calibration report shape | Flags and report section exist | CLI snapshot + unit test of calibration aggregate |

## Documentation impact

- `docs/cli/code-review.md` / generated `commands.json` (via `cargo xtask docs-cli`)
- `plugins/code-review/README.md` — eval usage, replay, quality bar
- Skill or command entry only if agents should invoke eval (thin pointer; primary
  surface remains `review` skill for production runs)
- This design + plan under `docs/toolu/`

## Open Questions

None blocking. Defaults settled by issue #670, epic #660, and brainstorm Jev
choices (`cli_eval`, `repo_fixtures`, `skip_judge`, `sha256_request`,
`matcher_only`, `claim_only`). Live full-matrix calibration may use replay
blobs recorded in a developer run; committing those blobs is in scope when size
is reasonable, otherwise the CLI documents how to record them without failing
the gate.
