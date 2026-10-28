# Code-review orchestration (#666) — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-code-review-orchestration-design.md   **Topic:** Prompt build, OpenRouter review call, Jev judge, effort selection, opt-in.

## Evidence and approach

Inspected: `crates/code-review/src/{budget,contract,model,memory,rules,prepass,shape}.rs`,
`crates/core/jev/src/{lib,retry,reply,question}.rs`, `crates/core/openrouter/src/*`,
`crates/core/runtime/src/config/load.rs`, `crates/epic-orchestrator/src/route/tier.rs`,
`crates/pr-babysit/src/route/jev.rs`, `crates/http-test-support` (loopback HTTPS with
body-needle routes), `crates/cli/tests/code_review_cli.rs`. Reuse: `model::request/parse`,
`Client::complete_structured(deadline)`, `Jev` reply parsing, loader `check_text`,
the tier bias. New code lives in `crates/code-review/src/review/` plus `settings.rs`;
`core/jev` gains additive APIs only.

## Workstream summary

core/jev transport → config + settings → contract/memory/budget types → mask, source,
prompt → effort → judge + recheck → `review::run` + CLI opt-in + black-box loopback
tests → docs + generated CLI docs → full gate.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"core/jev: from_env_key, ask_until shared deadline, request_bytes, OpenRouter constants, Usage.cost_usd","check":"cargo test -p toolu-jev-client && cargo test -p toolu-documentation-gate","ac_refs":["AC-8"],"paths":["crates/core/jev","crates/documentation-gate/src"],"input":"loopback Jev endpoint (http-test-support) with delayed and 5xx replies","model":"inherit"},
  {"id":"s2","title":"Config: codeReview known key; code-review settings (project-only enabled, merged effort)","check":"cargo test -p toolu-runtime config && cargo test -p toolu-code-review settings","ac_refs":["AC-1"],"paths":["crates/core/runtime/src/config","crates/code-review/src/settings.rs","crates/code-review/src/tests/settings_test.rs"],"input":"temp user/project toolu.config.json files","depends_on":["s1"],"model":"inherit"},
  {"id":"s3","title":"Contract/memory/budget: ErrorKind not-enabled+deadline, PriorFinding path/category/claim, Memory, PROMPT caps, judge deadlines; CLI schema enum","check":"cargo test -p toolu-code-review && cargo test -p toolu-cli --test contract","ac_refs":["AC-11"],"paths":["crates/code-review/src","crates/code-review/tests","crates/cli/src/commands.schema.json","crates/cli/tests"],"depends_on":["s2"],"model":"inherit"},
  {"id":"s4","title":"review/mask, review/source, review/prompt with unit tests","check":"cargo test -p toolu-code-review review::","ac_refs":["AC-2","AC-10","AC-15"],"paths":["crates/code-review/src/review"],"input":"temp git repos with modified/added files, a committed secret, a base-ref override","depends_on":["s3"],"model":"inherit"},
  {"id":"s5","title":"review/effort: floors, Jev score selector, fallback, explicit levels","check":"cargo test -p toolu-code-review effort","ac_refs":["AC-7","AC-13","AC-14"],"paths":["crates/code-review/src/review"],"input":"shaped temp repos per floor; loopback Jev score replies","depends_on":["s4"],"model":"inherit"},
  {"id":"s6","title":"review/judge (batching, caps, uncertainty, deadline) and review/recheck (dismissal validation)","check":"cargo test -p toolu-code-review judge && cargo test -p toolu-code-review recheck","ac_refs":["AC-3","AC-4","AC-5","AC-6","AC-12"],"paths":["crates/code-review/src/review"],"input":"loopback Jev + OpenRouter replies; >90,000-byte evidence","depends_on":["s5"],"model":"inherit"},
  {"id":"s7","title":"review::run orchestration, usage/calls/recorder, CLI opt-in; black-box review_contract tests","check":"cargo test -p toolu-code-review --test review_contract && cargo test -p toolu-cli --test code_review_cli","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-9","AC-10","AC-11","AC-12","AC-14","AC-15"],"paths":["crates/code-review","crates/cli/tests/code_review_cli.rs"],"input":"temp git repo diff + loopback OpenRouter and Jev origins","depends_on":["s6"],"model":"inherit"},
  {"id":"s8","title":"Docs: config.md codeReview, example config, code-review README, regenerated docs/cli","check":"cargo xtask docs-cli --check && cargo xtask check-markdown-cli","ac_refs":["AC-1","AC-15"],"paths":["docs/config.md","plugins/toolu/settings/toolu.config.example.json","plugins/code-review/README.md","docs/cli"],"depends_on":["s7"],"model":"inherit"},
  {"id":"s9","title":"Full quality gate","check":"cargo xtask gate","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-8","AC-9","AC-10","AC-11","AC-12","AC-13","AC-14","AC-15"],"depends_on":["s8"],"model":"inherit"}
]
```

## Critical files

- `crates/core/jev/src/{lib,retry,reply}.rs`, `crates/core/jev/tests/deadline.rs`
- `crates/core/runtime/src/config/load.rs`
- `crates/code-review/src/{lib,cli,settings,budget,contract,memory,review}.rs`
- `crates/code-review/src/review/{mask,source,prompt,effort,judge,recheck,calls}.rs` + `recheck.schema.json`
- `crates/code-review/src/review/tests/*_test.rs`, `crates/code-review/tests/review_contract.rs`
- `crates/cli/src/commands.schema.json`, `crates/cli/tests/code_review_cli.rs`
- `docs/config.md`, `plugins/toolu/settings/toolu.config.example.json`, `plugins/code-review/README.md`, `docs/cli/*`

## Verification

Loopback HTTPS origins (real TLS, real `toolu_http`) stand in for openrouter.ai; the
diff comes from real temp git repos through `shape`, `rules::gather` and `prepass::run`.
Boundaries: zero requests without opt-in, oversize evidence, Jev outage, deadlines,
invalid dismissals. Docs synchronized via `docs-cli --check` and `check-markdown-cli`;
`cargo xtask gate` last.

Delivery: scoped `feat(code-review):` commits pushed after each green step; final
`toolu ledger run <plan> --verify`, `toolu-review:review`, `toolu ledger verdict status`
= ready, push, PR `Closes Falconiere/toolu#666` / `Part of Falconiere/toolu#660`,
then `toolu epic report babysit --pr N` (epic worker handoff instead of babysit).

## Plan review (2026-10-27)

Jev step/AC alignment 0.51 (uncertain): the black-box step s7 now also cites the ACs its loopback tests prove (AC-2, AC-5, AC-7, AC-15). Every AC maps to a step with a real-input check; docs have s8; dependencies are linear. Approved.
