# Code-review verdict and state — Design

**Date:** 2026-10-28   **Status:** Approved   **Author:** Codex   **Topic:** #669 local verdict and gate state

## Problem

The pipeline can shape, scan, review, and validate a committed diff, but its public command still returns `unconfigured`. Agents need a compact verdict and gate-compatible proof without seeing full evidence or falsely passing incomplete reviews.

## Non-Goals

1. GitHub comments, automatic fixes, and a new MCP server.
2. Changing the push-review gate's path comparison or round cap.
3. Writing the repository opt-in for a user.

## Architecture

`run` composes the existing stages and derives the verdict from validated findings and coverage. The v2 writer moves from `toolu-review` to `toolu-state`; both callers use one recipe. A separate locked per-branch code-review file stores prior findings, dismissals, and recent passes. State paths use the host state root. A successful full committed-diff run writes push state; a path-limited, dirty, empty, or incomplete run cannot write a passing state. The CLI and skill show compact output; JSON retains the report contract.

## Interfaces / Schema

- `toolu code-review run [--base REF] [--path PATH]... [--effort auto|small|medium|high|xhigh]` exits 0 approved, 2 blocked, 69 unconfigured/outage/deadline, 64 usage.
- `toolu code-review dismiss <finding-id> --reason TEXT` records an explicit dismissal in branch memory without a model call.
- Push state v2 has `version`, `branch`, `diff_sha`, `base_branch`, `reviewed_at`, `reviewers`, `findings_count`, `findings`, `review_round`, `reviewed_files`. Reviewers include `code-review` and `code-review:native`; `findings_count` counts medium and above. Both writers replace the same `<slug>.json`; the last wins. Same diff increments the round without capping; the gate escalates on round 6.
- Memory state is a separate per-branch JSON file in the host state root with its `diff_sha`, `prior`, `dismissals`, and recent `passes`. Findings match by ID and line-independent fingerprint; dismissed blockers require an exact ID.
- Agent output includes one checklist line with effort level and source, worst-first findings, nonempty coverage/recap sections, Mechanical checks on outage, and usage with serving provider and quantization for each call. Append-only usage JSONL contains tokens, cost, model, provider, quantization, timing, and outcome only.

## Failure modes and edge cases

Missing consent or key sends nothing and exits 69 with a distinct `error.kind`. A total model outage emits a blocking finding and Mechanical checks, then exits 69. Missing history causes a full review. A malformed memory file fails closed. A dirty or path-scoped tree cannot write passing gate state. Noise-policy skips count in the gate path set and remain named under `coverage.skipped`; unreviewed paths block. Git's text-mode path quoting, rather than raw shaper paths, is written to `reviewed_files`. Lock acquisition failures cannot silently corrupt memory.

## Acceptance criteria

- **AC-1:** A clean, complete committed diff prints an approved compact verdict and writes v2 state that the real push-review gate accepts.
- **AC-2:** A diff with validated medium-or-higher findings prints worst-first findings, exits 2, and records a nonzero blocking count; low/nit remain advisory.
- **AC-3:** Total model outage exits 69 with a blocking finding and Mechanical checks; no passing state is written.
- **AC-4:** Same-diff rewrites increment `review_round`; round 6 makes the gate escalate. A later `toolu review write-state` overwrites the same gate file without erasing code-review memory.
- **AC-5:** Review memory persists prior findings and pass history by `diff_sha`; an explicit dismissal records its reason without a network call.
- **AC-6:** Path-scoped, dirty, empty, or unreviewed coverage cannot produce passing gate state; noise-policy skips remain listed with reasons.
- **AC-7:** The thin skill and docs explain opt-in, outbound data, effort, host requirements, and command use; generated CLI docs and packaging/host checks pass.
- **AC-8:** JSON gives the agent only compact validated findings, coverage, recap, and per-call usage/provider metadata; the usage log contains no source, quote, or key.
- **AC-9:** The skill never fans out to subagents; supported hosts expose only a thin command entry, with the skill as the primary surface.

## Acceptance evidence

- AC-1/2/3: black-box committed fixture repositories using real Git, recorded provider responses and controlled outage, checked with `cargo test -p toolu-code-review` and the gate.
- AC-4/5: real Git state rewrites and `toolu review write-state`, including Unicode paths, checked by crate integration tests.
- AC-6: fixture repo with `Cargo.lock`, a reviewed file, path scope, dirty tree, and unreviewed coverage; checked by integration tests.
- AC-7: `cargo xtask docs-cli`, `cargo xtask check-markdown-cli`, packaging validation, host smoke, and `cargo xtask gate`.
- AC-8: inspect a real call's report and append-only usage log, asserting provider/quantization and absence of source, quote, and key.
- AC-9: inspect and validate installed plugin assets on Claude, Codex, and Muse with `.toolu/skills/verify-dual-host-plugins/SKILL.md`.

## Documentation impact

Update `plugins/code-review/README.md`, `docs/config.md`, generated `docs/cli/commands.json`, and the new `plugins/code-review/skills/` skill.

## Open Questions

None. The issue and epic decisions settle the contract and the user supplied the offline delivery cycle.
