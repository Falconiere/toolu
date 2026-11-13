# Finished detached job notice — Plan

**Date:** 2026-11-13   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-13-finished-detached-job-notice-design.md   **Topic:** Issue #878

## Evidence and approach

Issue #878 and epic #869 settle the hook surfaces and notice text. The merged #872 result module already provides atomic records, worktree binding, claim-once delivery, and line formatting. Reuse those APIs and the existing advisory encoder; keep the prompt's skip and block paths intact.

## Workstream summary

Shared claim and post-tool delivery, then prompt delivery and user docs, followed by the full gate and committed-diff audit.

## Steps (machine-readable)

```json
[
  {
    "id": "post-tool",
    "title": "Claim and report finished jobs after edit or shell calls",
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "paths": ["crates/core/engine/src/resources/job_results.rs", "crates/core/engine/src/gates/job_results.rs", "crates/core/engine/src/gates.rs", "crates/core/engine/src/builtins.rs", "crates/core/engine/src/dispatch/fold.rs", "crates/core/engine/src/dispatch/output.rs", "crates/core/engine/tests/post_waiver.rs", "crates/cli/tests/tool_hooks.rs", "crates/cli/tests/helpers/tool_hooks_jobs.rs", "crates/xtask/src/tests/gate_coverage_names_test.rs"],
    "input": "Real temporary Git worktree bindings and finished result JSON, including a failing result, another worktree, a later blocking post-tool module, and a later exit-2 module",
    "check": "cargo test -p toolu-cli --test tool_hooks"
  },
  {
    "id": "prompt",
    "title": "Append a finished job to the next normal prompt hint",
    "ac_refs": ["AC-3", "AC-4"],
    "depends_on": ["post-tool"],
    "paths": ["crates/toolu/src/lifecycle/prompt_submit.rs", "crates/toolu/src/lifecycle/tests/prompt_submit_test.rs", "crates/core/engine/src/resources/job_results.rs"],
    "input": "Real temporary Git worktree binding and failed finished result; normal, trivial, and blocked prompt payloads",
    "check": "cargo test -p toolu-hub --lib -- prompt_submit"
  },
  {
    "id": "docs",
    "title": "Document the once-only detached job notification",
    "ac_refs": ["AC-1", "AC-3"],
    "depends_on": ["prompt"],
    "paths": ["docs/epic-orchestrator/README.md", "docs/gate-coverage-matrix.md", "fixtures/gate-coverage/inventory.json"],
    "input": "The documented toolu epic job --detach usage and hook behavior",
    "check": "cargo xtask check-markdown-cli"
  },
  {
    "id": "gate",
    "title": "Run the complete local gate and hook latency assertions",
    "ac_refs": ["AC-5"],
    "depends_on": ["docs"],
    "input": "The final branch diff and real hook benchmark fixtures",
    "check": "cargo xtask bench-hooks --only toolu/post-tools --only toolu/user-prompt-submit --assert && cargo xtask gate"
  }
]
```

## Critical files

`crates/core/engine/src/resources/job_results.rs`, `crates/core/engine/src/gates/job_results.rs`, `crates/core/engine/src/gates.rs`, `crates/core/engine/src/builtins.rs`, `crates/core/engine/src/dispatch/{fold,output}.rs`, `crates/core/engine/tests/post_waiver.rs`, `crates/cli/tests/{tool_hooks.rs,helpers/tool_hooks_jobs.rs}`, `crates/toolu/src/lifecycle/prompt_submit.rs`, `crates/toolu/src/lifecycle/tests/prompt_submit_test.rs`, `crates/xtask/src/tests/gate_coverage_names_test.rs`, `docs/epic-orchestrator/README.md`, `fixtures/gate-coverage/inventory.json`, and `docs/gate-coverage-matrix.md`.

## Verification

The post-tool test must assert the exact one-time host advisory line, an empty second call, an unclaimed other-worktree record, a later block whose visible reason includes the claimed notice, and a later exit-2 whose stderr includes it. The prompt test must assert the failed result's line followed by its stored output in `additionalContext`, an empty repeat, and no claim on trivial or blocked prompts. Run `cargo xtask check-changed --base origin/main` after each code step. After scoped commits, run `toolu ledger run <plan> --verify`, review the committed diff, require a ready ledger verdict, push the branch, and open the issue PR. The worker brief substitutes its `report babysit` handoff for the skill's babysit invocation.

## Deviations

- Before execution, moved the prose step from generated `docs/cli/epic.md` to maintained `docs/epic-orchestrator/README.md`; the generated reference is unchanged because no CLI flag changes.
- During the first behavior step, added the post-block visibility case after finding that a later block otherwise discards a claimed advisory.
- The changed-path gate required registering the new built-in in the coverage inventory and matrix. It also found duplicate test setup and Clippy findings in the new test helper, corrected before rerun.
- The changed-path workspace run found the existing `POST_TOOL` order assertion needed the new built-in trace row.
- The later workspace run found `xtask`'s discovery order assertion also needed the new built-in row.
