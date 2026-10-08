# Pre-tool gates C in Rust — Design

**Date:** 2026-10-08   **Status:** Approved   **Author:** Codex   **Topic:** Port #422 workflow gates and standalone hooks to the native `toolu` binary.

## Problem

The Rust pre-tool dispatcher still omits push-review, plan-ledger, and docs-sync. Its native binary also lacks the agent-tier and `mcp__` hook entries. A native host switch would therefore lose review and plan enforcement, documentation advice, delegation routing advice, and MCP blocking.

## Non-Goals

1. Switching `hooks.json` to native entries; #425 owns the rollout.
2. Changing gate presets, review or ledger schemas, docs-sync defaults, telemetry opt-outs, or existing policy text.
3. Altering `toolu ledger verdict`'s report-only behavior or unrelated plugin hooks.

## Architecture

Add `Gate` implementations for `docs-sync`, `plan-ledger`, and `push-review` in TypeScript's built-in order. Share a Rust push-target helper over `toolu-shell` analysis and `toolu-state` git facts. Reuse #421's ledger jq, AC coverage, review-state, waiver, and docs glob primitives, while keeping hook-specific decisions, text, telemetry, and modes in dedicated modules. Add native `agent-tier` and `mcp-tools` hook adapters in the hub, routed by `crates/cli` through the existing hook path. The MCP adapter invokes the existing mcp-blocker gate without shell parsing. Agent-tier parses the raw payload and joins the checked-out branch's ledger before recording telemetry and producing a host-encoded decision.

## Interfaces / Schema

- `toolu hook pre-tools` includes nine built-ins in the established order: bash-commands, code-edit-rules, commit-gate, docs-sync, mcp-blocker, plan-ledger, protected-files, push-review, quality-gate.
- `toolu hook agent-tier` and `toolu hook mcp-tools` accept the same stdin payload and optional `--event`/`--plugin-root` flags as other hook entries. They return the existing Claude/Codex command JSON, stderr warnings, and exit code.
- Review state remains v2 under `push-review/<branch-slug>.json`; ledger state remains v1 under `plan-ledger/<branch-slug>.json`; docs attestation remains under `docs-sync/<branch-slug>.json`. Existing environment overrides (`STATE_DIR`, `LEDGER_DIR`, `DOCS_SYNC_STATE_DIR`, `PUSH_REVIEW_BASE`, `DOCS_SYNC_BASE`) and config sections (`gates`, `docsSync`, `planLedger`) retain their meanings.
- `TOOLU_IMPL=rust:toolu/pre-tools`, `rust:toolu/agent-tier`, and `rust:toolu/mcp-tools` select native entries through the existing conformance seam. Adapt `mcp-tools.test.ts` to call `launchedArgv` for functional cases instead of reading the `hooks.json` command directly; keep its bundle-specific shell-parser assertion on the Bun bundle.

## Failure modes and edge cases

- A command without a parsed `git push`, a missing git executable, a base branch push, and an unanalyzable oversize shell input do not trigger the three workflow gates. A `git -C` push judges that repository and its destination branch.
- Push-review preserves its exact check order: absent base, detached HEAD, empty diff, waiver, absent or malformed state, v1/schema, reviewer, round cap, stale SHA, open findings, and reviewed-file coverage. Ask mode writes a pending waiver for a known diff; a passing waiver permits the push.
- Plan-ledger permits an absent or empty ledger, rejects an unparseable ledger, checks schema and optionally uncovered ACs, then requires fresh-green steps and whole-diff verification. It never runs a step during a push.
- Docs-sync permits a doc surface, a non-code diff, mode off, or an attestation for the current SHA; otherwise it writes the same telemetry and emits the configured advisory/ask/deny. Malformed config warnings and invalid state retain current behavior.
- Agent-tier fails open on malformed payload, missing git, unreadable ledger, or telemetry errors. It records a delegation when possible and compares only an explicit model to the running step's model, else the ledger's next step. MCP permits non-MCP/malformed input and no-source calls without loading config; a listed server uses mcp-blocker policy and reports config warnings.
- Host encoding keeps Codex ask degradation, and module failures retain the existing dispatcher's fail behavior. No new state format or migration is introduced.

## Acceptance criteria

- **AC-1:** Each of the 164 pre-tools cases in `fixtures/gates/pre-tool-modules-c.json` produces its expected decision and matches the committed non-deviation golden's parsed stdout, stderr, exit, and touched files under the Rust binary.
- **AC-2:** Each of the 16 agent-tier cases in the C fixture produces its expected host decision and state effects under `toolu hook agent-tier`, including mismatched tier and missing model.
- **AC-3:** Native `toolu hook mcp-tools` asks on Claude and denies on Codex for a listed MCP server, permits unlisted or malformed calls, and reports malformed config as today's standalone hook does.
- **AC-4:** The complete A/B/C pre-tool suite passes under `TOOLU_IMPL=rust:toolu/pre-tools`, including existing A/B gates, full built-in ordering, and the oversize shell boundary.
- **AC-5:** Rust unit and integration tests exercise real git repositories, exact review/ledger/docs state reads and writes, host modes, and native CLI routing without test doubles; the full Rust and Bun gates pass.

## Acceptance evidence

| AC | Representative real input and expected result | Boundary or failure case | Runnable check |
|---|---|---|---|
| AC-1 | A branch push with a matching clean review passes; missing review, stale SHA, and uncovered ledger AC return the captured decisions and state writes | `git -C`, refspec, malformed state, config and docs attestation | `TOOLU_IMPL=rust:toolu/pre-tools TOOLU_RUST_BIN_DIR=$PWD/target/release bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-c.test.ts` |
| AC-2 | Delegation naming a model different from a running ledger step emits the configured host decision and delegation telemetry | Missing model, malformed ledger, no git | Same C fixture runner with `rust:toolu/agent-tier` enabled |
| AC-3 | `mcp__exampleblocked__search` with a real blocklist asks on Claude and denies on Codex | Empty/non-JSON/array payload, unlisted server, malformed config | `TOOLU_IMPL=rust:toolu/mcp-tools TOOLU_RUST_BIN_DIR=$PWD/target/release bun test plugins/toolu/hooks/src/__tests__/mcp-tools.test.ts` |
| AC-4 | All A, B, and C fixture runners replay real sandbox commands through the native binary | Oversize shell input and earlier gate order | `TOOLU_IMPL=rust:toolu/pre-tools TOOLU_RUST_BIN_DIR=$PWD/target/release bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-{a,b,c}.test.ts` |
| AC-5 | Rust tests run hook entry points against real git and state paths; complete quality gates finish clean | Malformed files and host-specific ask encoding | `cargo xtask gate --base origin/main --title 'feat(engine): port pre-tool gates C to Rust (#422)'` and `bun run test` |

## Documentation impact

Update `docs/registry.md` and its generated OpenCode copy to describe the completed nine native built-ins and the two standalone native hook entries. Keep the #425 host-switch boundary explicit. Regenerate CLI references if the hook command tree changes.

## Open Questions

None. The issue, merged dependencies, TypeScript source, and committed fixtures specify the observable contract.
