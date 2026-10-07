# Pre-tool gates B in Rust — Design

**Date:** 2026-10-07   **Status:** Approved   **Author:** Codex   **Topic:** Port bash-commands, commit-gate, and quality-gate to the Rust pre-tool dispatcher.

## Problem

The native pre-tool dispatcher from #418 still lacks three command-driven gates. A Rust hook could therefore permit a denied shell command, an invalid commit subject, or a commit or push while the recorded quality gate fails. Issue #420 requires the Rust path to reproduce the current TypeScript gates before #425 switches hosts to it.

## Non-Goals

1. Switching `hooks.json` to the native hook; #425 owns that.
2. Changing the shipped deny lists, Conventional Commit policy, gate modes, or gate-file schema.
3. Porting the remaining pre-tool gates assigned to #422.

## Architecture

Add three `Gate` implementations under `crates/core/engine/src/gates/` and place them in their TypeScript built-in slots. The full table order is `bash-commands`, `code-edit-rules`, `commit-gate`, `docs-sync`, `mcp-blocker`, `plan-ledger`, `protected-files`, `push-review`, `quality-gate`; the #422 slots stay absent until that issue lands. Use `command_analysis` and `toolu-shell`'s per-command argv rules and git invocation parser. Use the existing runtime settings and gate-mode loaders, `toolu-state` for gate-file reads and git branch/worktree facts, and `Roots` for host-specific state paths. The engine emits the same `Decision` and wording as the TypeScript gates.

The dispatcher and CLI remain unchanged except for the populated built-in table. The current Bun hook remains the host entry until #425. `TOOLU_IMPL=rust:toolu/pre-tools` selects the native binary in the conformance harness, so the committed case runner can exercise this implementation directly.

## Interfaces / Schema

- `Gate::run(&NormalizedEvent, &RuleContext) -> Result<Decision, String>` is the existing module contract. Gate names stay `bash-commands`, `commit-gate`, and `quality-gate`.
- `toolu-runtime::config::settings::{BASH_ALLOWLIST, BASH_DENYLIST, COMMIT_PREFIXES, read_list}` supplies settings with the existing file-order semantics. A deny rule with no ASCII space searches a simple command's source text; a multiword rule uses `toolu_shell::rules::matches_rule` on written and unwrapped argv. An allow hit overrides a deny hit only for that simple command.
- A commit subject's first static `-m` or `--message` paragraph is checked for the existing lowercase `type(scope):` prefix. Dynamic messages receive the reminder without an invented prefix. The reminder's base branch comes from `toolu_state::git::base_branch`.
- Quality state is `<project>/<host state dir>/tmp/quality-gate-status.json`. `toolu_state::gate_file::read_gate_file` classifies it. Both strict `GateRead::Ok` and parseable legacy `GateRead::Unrecognized` values supply top-level `status`, `reason`, and `violations` as TypeScript does; no state-file mutation occurs in a pre-tool gate. The existing PostToolUse gate-status writer supplies the Rust-side write in the interleaving test.

## Failure modes and edge cases

- With no deny rules, `bash-commands` is silent; with rules and an unknown shell analysis, it follows the configured ask/block/advise mode and includes the parser reason. A rule that matches only a commit message, heredoc body, or a different simple command does not create a false hit.
- A non-commit tool or command never loads commit settings. A commit with no prefix file, a valid prefix, or a dynamic message gets the existing reminder; a static unlisted prefix follows the configured mode. Codex ask degradation and invalid-config behavior remain those of the runtime gate mode.
- `quality-gate` reads state only for a possible commit/push. `MY_CLAUDE_QUALITY=off`, mode `off`, missing git, a linked worktree, absent/malformed/non-failing state, and unrelated tools leave it silent. A parseable failing legacy file still blocks or advises by mode. Null/false/absent reason and violations use the existing fallbacks; object and array values render as `jq -r` does.
- A settings read error propagates through `Gate::run` to the dispatcher's module-error reporting. Gate-file reads and pre-tool checks do not write or lock the file, so TypeScript and Rust post-tool writers keep #415's lock and byte format.

## Acceptance criteria

- **AC-1:** Selecting `toolu/pre-tools` through the `TOOLU_IMPL` harness runs every case in `fixtures/gates/pre-tool-modules-b.json` against the Rust binary, reproduces each case's expected decision and text, and matches the committed TypeScript golden's decision JSON, stderr, and exit code for cases without a named historical deviation.
- **AC-2:** The #283 shell cases for these gates deny `cd /tmp && node -e 1` when listed, recognize a static commit reached through wrappers, and block commits/pushes reached through wrappers while quality state fails; benign lookalikes remain allowed.
- **AC-3:** In a real git sandbox, a TypeScript post-tool failure is visible to a Rust pre-tool commit check; a Rust post-tool failure is visible to a TypeScript pre-tool commit check; clearing the gate through either writer permits the next commit check.
- **AC-4:** The Rust pre-tool built-in order and host modes preserve the existing decisions, including an allow override on one simple command, the Conventional Commit reminder, Codex ask degradation, legacy gate-file values, and linked-worktree handling.

## Acceptance evidence

| AC | Representative real input and observable result | Boundary or failure case | Runnable check |
|---|---|---|---|
| AC-1 | All 110 committed B cases run through `TOOLU_IMPL=rust:toolu/pre-tools`; non-deviation cases match `pre-tool-modules-b-golden.json` | Named #283 deviations differ from the known-wrong historical capture; missing settings, malformed state/config, Claude and Codex | `TOOLU_IMPL=rust:toolu/pre-tools TOOLU_RUST_BIN_DIR=/root/.local/bin bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-b.test.ts` |
| AC-2 | `cd /tmp && node -e 1` with shipped deny list is denied; wrapped commit/push fixtures return the expected decision | Commit message mentioning a denied command and heredoc data remain benign | The same B fixture runner, filtering or logging the named `#283` cases |
| AC-3 | Two hooks, two implementations, one real gate file and git repository; failure blocks and clear permits | Strict multi-slot state and stale/legacy state | `cargo test -p toolu-engine --test pre_gates_interleave` |
| AC-4 | The built-in table emits the expected allow/advisory/ask/deny for each gate in order | Empty list, dynamic message, linked worktree, Codex ask | `cargo test -p toolu-engine --test pre_gates` and the B fixture runner |

## Documentation impact

Update `docs/registry.md` to name these Rust built-ins and their host-switch boundary. Gate policy and user settings remain as documented in `plugins/toolu/hooks/docs/gates.md`.

## Open Questions

None. The issue, merged engine, committed fixtures, and existing TypeScript gates fix the observable behavior.
