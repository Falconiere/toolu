# Code-review paired-comparison eval harness (#670) — Plan

**Date:** 2026-10-28   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-28-code-review-eval-harness-design.md   **Topic:** Expectation-based paired eval with offline matcher and explicit live/replay CLI.

## Evidence and approach

Issue #670 and epic #660 require expectation-judged quality, not fewer tokens.
#666 already reserved `Options.skip_judge` (baseline) and `Options.recorder`.
Existing crate tests build real Git temp repos and drive OpenRouter/Jev over
`toolu_http_test_support::Fixture` loopback. Spec: matcher/report offline in
gate; `toolu code-review eval` for replay/live; fixtures under
`fixtures/code-review-eval/` with materializable mini-repos (file trees → temp
`git init`, matching `fixture_test.rs`).

Replay transport (decided for s3): add an optional `Options.replay` lookup
consulted inside `review::calls::Ledger` **before** any network call. The
lookup key is `stage` + SHA-256 of the same masked request string the recorder
emits. A hit returns the stored body and marks the `Call` as replayed with
zero billed cost; a miss in non-live mode fails the case closed. `--live`
calls the real clients, then the existing `recorder` writes/updates
`replay/<stage>-<sha256>.json`. Offline gate tests never set `replay` or
`recorder` on a network path — they only unit-test the store and matcher.

## Workstream summary

Core matcher + report + fixtures → replay store and eval runner → CLI/docs →
full gate.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"Add eval types: expectations schema, path+claim matcher, report with unclassified/live-or-replayed, tokens-per-finding; offline unit tests","check":"cargo test -p toolu-code-review --lib eval::","ac_refs":["AC-1","AC-3","AC-8"],"paths":["crates/code-review/src/eval.rs","crates/code-review/src/eval","crates/code-review/src/lib.rs"],"input":"synthetic Finding lists and expectation rows covering defect match, false_positive flag, unclassified leftover","model":"inherit"},
  {"id":"s2","title":"Commit fixtures/code-review-eval golden set (probe defects, cross-file, prompt FP, memory seeds) and materialize-git helper; offline tests for load + memory + gate-contract predicates","check":"cargo test -p toolu-code-review --lib eval:: && cargo test -p toolu-code-review --test eval_contract","ac_refs":["AC-2","AC-5","AC-6","AC-8"],"paths":["fixtures/code-review-eval","crates/code-review/src/eval","crates/code-review/tests/eval_contract.rs","crates/code-review/src/pipeline/state.rs","crates/code-review/src/contract.rs"],"input":"committed case file trees + expectations.json + memory.json seeds; Severity::blocking and mark_coverage/complete predicates","depends_on":["s1"],"model":"inherit"},
  {"id":"s3","title":"Add Options.replay pre-network lookup + replay store; wire eval runner arms; CLI code-review eval (--live/--replay-dir/--effort including none/auto); calibration aggregate unit test; black-box no-network default and missing-key live exit 69","check":"cargo test -p toolu-code-review --test eval_contract && cargo test -p toolu-cli --test code_review_cli","ac_refs":["AC-3","AC-4","AC-9","AC-8"],"paths":["crates/code-review/src/eval","crates/code-review/src/cli.rs","crates/code-review/src/lib.rs","crates/code-review/src/review/calls.rs","crates/code-review/src/review.rs","crates/cli/tests/code_review_cli.rs","crates/cli/tests/fixtures/commands_json.snap","crates/cli/src/commands.schema.json"],"input":"replay blob hit (cost 0, live_or_replayed=replayed) and miss; calibration fixture rows; CLI without --live; CLI --live with OPENROUTER_API_KEY unset","depends_on":["s2"],"model":"inherit"},
  {"id":"s4","title":"Document quality bar and eval usage; regenerate CLI docs; packaging/markdown checks","check":"cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo xtask check-hooks","ac_refs":["AC-7","AC-8"],"paths":["plugins/code-review/README.md","docs/cli/code-review.md","docs/cli/commands.json","docs/cli/commands.schema.json","docs/cli/commands.md"],"input":"generated command help for code-review eval","depends_on":["s3"],"model":"inherit"},
  {"id":"s5","title":"Full local quality gate on final branch diff","check":"toolu epic job -- sh -c 'export PATH=/opt/homebrew/bin:$PATH; cargo xtask gate'","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-8","AC-9"],"paths":["crates/code-review","crates/cli","fixtures/code-review-eval","plugins/code-review","docs/cli","docs/toolu/plans/2026-10-28-code-review-eval-harness.md"],"input":"real final branch diff against origin/main","depends_on":["s4"],"model":"inherit"}
]
```

## Critical files

`crates/code-review/src/eval*` (new), `crates/code-review/src/{cli,lib,review}.rs`,
`crates/code-review/src/review/calls.rs`, `crates/code-review/tests/eval_contract.rs`,
`fixtures/code-review-eval/`, CLI snapshot/schema, `plugins/code-review/README.md`,
generated `docs/cli/*`.

## Verification

Offline matcher and gate-contract tests use real fixtures and production
predicates. CLI tests prove no network without `--live` and exit 69 when live
lacks a key. Replay hit marks `replayed` and does not add cost. Delivery:
`toolu ledger preflight` before edits; after s5, `toolu ledger run … --verify`,
`cargo xtask gate`, push-review verdict `overall: ready`, push, PR with
`Closes Falconiere/toolu#670` / `Part of Falconiere/toolu#660`, then babysit
handoff per the epic brief.

## Deviations

- `--effort none` runs the baseline arm only at a small budget, because the OpenRouter client rejects reasoning `none`; it is not a true reasoning-off run.
- Fixture sources are `*.rs.fixture` files that materialize to `*.rs`, so `check-reach` stays clean.
- s5's check is only the quality gate (not nested `ledger --verify`), so verify can stamp every step without recursion. Homebrew `python3` is put first on `PATH` under the epic job so `tomllib` is available on this macOS host.
