# Code-review verdict and state (#669) — Plan

**Date:** 2026-10-28   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-28-code-review-verdict-state-design.md   **Topic:** Local verdict, branch memory, gate state, skill and docs.

## Evidence and approach

Issue #669 and epic #660 define severity, fail-closed behavior, memory, consent and gate fields. The existing v2 writer is `crates/toolu-review/src/state.rs`; the gate compares text-mode Git names, while the shaper gives raw NUL paths. Move the recipe to `toolu-state`, share it with both plugins, and convert names at the boundary. Keep code-review memory separate and locked.

## Workstream summary

Shared writer and real-Git contract → pipeline composition, memory and compact report → plugin skill/docs and host verification → full local gate and review.

## Steps (machine-readable)

```json
[
  {"id":"s1","title":"Share the v2 state writer in core/state; preserve toolu-review behavior and prove Git path spelling/round semantics","check":"cargo test -p toolu-state && cargo test -p toolu-review","ac_refs":["AC-1","AC-4","AC-6"],"paths":["crates/core/state/src/push_review.rs","crates/core/state/src/tests/push_review_test.rs","crates/toolu-review/src/state.rs","crates/toolu-review/src/writer.rs"],"input":"real temporary Git branch with Unicode path and repeated same-diff writes","model":"inherit"},
  {"id":"s2","title":"Compose review stages, derived compact verdict, outage behavior, metadata-only usage and separate locked memory/dismissal","check":"cargo test -p toolu-code-review && cargo test -p toolu-cli --test code_review_cli","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-8"],"paths":["crates/code-review/src/lib.rs","crates/code-review/src/report.rs","crates/code-review/src/memory.rs","crates/code-review/src/contract.rs","crates/code-review/src/cli.rs","crates/code-review/tests"],"input":"committed fixture repos, recorded provider response and controlled provider outage","depends_on":["s1"],"model":"inherit"},
  {"id":"s3","title":"Ship lean skill and thin host entries, update user docs and generated CLI snapshot, verify packaging","check":"cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo xtask check-hooks","ac_refs":["AC-7","AC-9"],"paths":["plugins/code-review","docs/config.md","docs/cli/commands.json","docs/cli/README.md"],"input":"native plugin assets installed or validated in supported hosts","depends_on":["s2"],"model":"inherit"},
  {"id":"s4","title":"Run all local quality gates and guardrails on final branch diff","check":"toolu epic job -- cargo xtask gate","ac_refs":["AC-1","AC-2","AC-3","AC-4","AC-5","AC-6","AC-7","AC-8","AC-9"],"paths":["crates/code-review","crates/toolu-review","crates/core/state","plugins/code-review","docs/cli","docs/config.md"],"input":"real final branch diff against origin/main","depends_on":["s3"],"model":"inherit"}
]
```

## Critical files

`crates/core/state/src/push_review.rs`, `crates/toolu-review/src/state.rs`, `crates/code-review/src/{lib,report,memory,contract,cli}.rs`, crate tests, `plugins/code-review/`, `docs/config.md`, generated CLI docs.

## Verification

Use real Git repositories and recorded network responses for clean, dirty, outage, unusual-path, round-cap, and incomplete-coverage cases. Run `toolu ledger preflight`, each focused check, the full gate, local `/code-review xhigh --fix`, then the user-specified rebase, gate, push and merge cycle. Final ledger verification and push-review verdict must be ready.
