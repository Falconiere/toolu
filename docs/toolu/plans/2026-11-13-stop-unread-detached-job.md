# Stop with an unread detached job — Plan

**Date:** 2026-11-13  **Status:** Approved  **Spec:** docs/toolu/specs/2026-11-13-stop-unread-detached-job-design.md  **Topic:** One Stop block per unread detached job.

## Evidence and approach

Issue #880 and merged #872 define the result and hook contracts. `job_results::pending` selects undelivered results by worktree; `with_resource_lock` serializes claims. The epic and ledger detached writers overwrite records they loaded before a Stop, so a separate per-id marker preserves the once-only decision without changing result records. Live Claude Code 2.1.323 and Muse Code 1.4.20 Stop probes blocked with a reason. Cursor Agent 2026.11.13 fired native `stop` interactively and made a second turn for `followup_message`; its headless Stop did not fire in the isolated probe. Codex, Hermes and OpenCode lack verified blocking Stop evidence. Jev selected the marker file and, after the Cursor probe, a Cursor-specific response. Keep ledger step freshness branch-wide by omitting optional `paths` rather than underdeclaring a cross-crate check.

## Workstream summary

Add the durable marker and protocol mapping, wire the CLI and generated manifests, update the hook documentation, then run the complete gate and delivery audit.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Persist one Stop warning without changing result delivery",
    "check": "unset PUSH_REVIEW_BASE; cargo test -p toolu-engine --lib -- resources::job_results::tests && cargo test -p toolu-protocol && cargo xtask check-changed --base origin/main",
    "ac_refs": ["AC-2", "AC-3"],
    "input": "real schema-1 waiting, running, done and claimed files across two temporary worktrees; concurrent Stop attempts; a warned pending result later claimed exactly once"
  },
  {
    "id": "s2",
    "title": "Block the first Stop through the real toolu binary and generated host hooks",
    "check": "unset PUSH_REVIEW_BASE; cargo test -p toolu-cli --test tool_hooks && cargo test -p toolu-cli --test stop_hook && cargo xtask check-hooks && cargo build --release --locked --bin toolu && cargo xtask bench-hooks --only toolu/stop --only muse:toolu/stop --assert && cargo xtask check-changed --base origin/main",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-4", "AC-5"],
    "input": "built toolu hook with real worktree binding and result files; generated Claude and Muse Stop hooks plus Cursor's shared-manifest mapping; benchmark payloads and budgets for both entries"
  },
  {
    "id": "s3",
    "title": "Document supported hosts and the once-only Stop behavior",
    "check": "unset PUSH_REVIEW_BASE; cargo xtask check-markdown-cli && cargo xtask check-hooks",
    "depends_on": ["s2"],
    "ac_refs": ["AC-4", "AC-5"],
    "input": "plugin hook manifest, generated Muse resolver, user-facing hook documentation and recorded live Claude/Muse/Cursor Stop probe evidence"
  },
  {
    "id": "s4",
    "title": "Run the complete quality gate",
    "check": "unset PUSH_REVIEW_BASE; cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "input": "final committed branch diff, real workspace test suite and generated hook benchmark"
  }
]
```

## Critical files

`crates/core/engine/src/resources/job_results.rs` and its sibling test; `crates/core/protocol/src/event.rs`, `native.rs` and their sibling tests; exhaustive `HostEvent` consumers in protocol; `crates/cli/src/hook.rs`, `hook/guard.rs`, `hook/stop.rs`, their sibling tests, `crates/cli/tests/tool_hooks.rs`, `stop_hook.rs`, and `native_toolu_manifest.rs`; `plugins/toolu/hooks/hooks.json`, `.muse-plugin/plugin.json`, `hooks/muse/stop`; `benchmarks/cases/hooks/payloads.json`, `benchmarks/hook-budgets.json`; `plugins/toolu/README.md`, the coverage inventory and matrix, and this spec and plan.

## Verification

The real-binary tests prove first block, named id, second pass, completed/unclaimed, cross-worktree and Cursor follow-up behavior. The engine test proves concurrency and later delivery after a pending Stop. `check-hooks` and `bench-hooks` prove packaging and latency. Install the generated plugin in an isolated Muse profile and run `muse plugins hook test toolu:stop --fixture <Stop JSON> --json`; require `should_block: true` and the id in `block_reason`. Run Claude Code with the checkout plugin in an isolated Stop-hook session; require an exit-2 reason followed by continuation. Record Cursor's interactive native `followup_message` success and headless gap accurately in the PR. Run `cargo xtask host-smoke --host both` to check the plugin's real host integration. Fetch and rebase if `origin/main` moves before implementation or push; rerun affected checks. Delivery requires the final ledger `--verify`, complete v2 code review, ready verdict, push and PR; the epic engine takes over after `report babysit`.

## Deviations

Cursor's interactive Stop probe established `followup_message` as its blocking response, so s2 now includes Cursor's native mapping and a separate real-binary test. The gate's colocated-test and hook-coverage rules require a sibling Stop module test plus the native manifest and coverage inventory updates.

The local `main` ref in this epic worktree lags `origin/main`. Ledger runs use `PUSH_REVIEW_BASE=origin/main` to stamp the issue diff; each check unsets that override so temporary Git repositories in the workspace tests resolve their own local base.
