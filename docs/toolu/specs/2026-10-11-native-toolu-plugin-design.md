# Native toolu plugin switch — Design

**Date:** 2026-10-11   **Status:** Approved   **Author:** Codex   **Topic:** Switch toolu host hooks and agent commands to the installed native binary.

## Problem

The toolu plugin still launches eight Bun hook entries even though their native counterparts exist. Each tool action pays the Bun startup cost. The plugin's debug skill also runs Bun scripts. Users upgrading the plugin may not yet have the native binary, so the switch needs the transition fallback agreed in epic #402 and issue #425.

## Non-Goals

1. Port other plugins or remove Bun from the repository; their bundles and the OpenCode adapter remain until their scheduled issues.
2. Remove the temporary Bun fallback or the TypeScript gates still imported by OpenCode; #440 and #437 own those removals.
3. Change gate decisions, state formats, registry ordering, or host permission policy.
4. Merge this PR or publish a release; the epic orchestrator owns merge and release-please owns publication.

## Architecture

Generate all eight existing `plugins/toolu/hooks/hooks.json` commands with `cargo xtask print-hook`, retaining their matchers and explicit timeouts. `toolu hook check-binary` uses the existing `toolu_runtime::startup::native_toolu_advice` so the separate SessionStart entry can be native; the other seven names already route to native handlers. The generated launcher selects the native binary first and supports `TOOLU_RUNTIME=bun` as an explicit rollback path. Its existing Bun fallback remains for at least one switched release, so the eight corresponding source and dist bundles remain shipped for that period. A missing native binary gives the existing install advisory, and a bad explicit `TOOLU_BIN` still fails closed.

Replace the planned `toolu debug` namespace with native `io`, `log`, `stack`, and `testfail` verbs. `io` reports the byte and line count of a file or stdin so an agent can size a captured input before summarizing it; the other verbs emit bounded text or one JSON document. Reuse the captured real Bun, Cargo, stack, and long-log fixtures. Update the debug skill and generated CLI reference. Remove only Bun sources, bundles, and TypeScript tests that have no transition, OpenCode, conformance, or tooling consumer, based on import and pack checks. Move ported hook entries into `fixtures/rust-ported.json` so the shared suites require Rust. Add the issue's `bun run test` command as an alias that runs the existing `test:unit` and `test:conformance` product suites. The notice-release commit #483 must be published before the switch release is merged; it is not yet contained in tag `v7.11.0`.

## Interfaces / Schema

- Hook argv: `toolu hook <name> --event <Event> --plugin-root <plugin dir>` for `session-start`, `check-binary`, `pre-compact`, `user-prompt-submit`, `pre-tools`, `mcp-tools`, `agent-tier`, and `post-tools`.
- Launcher environment: unset `TOOLU_RUNTIME` selects native, then the shipped Bun fallback when native is absent; `TOOLU_RUNTIME=bun` selects the shipped Bun bundle; `TOOLU_BIN` remains an exclusive native path when native selection is active. The rollback selector never silently runs a different bundle if the named one is absent.
- Debug CLI: `toolu debug io|log|stack|testfail [--file <path>]` with the global `--json` option. `io` returns `{bytes,lines}` (a final newline does not open another line); `log` returns `{errors,tail,total_lines,truncated}`; `stack` returns `{app_frames,noise_frames,recognized}`; `testfail` returns `{failures,errors,locations,recognized}`. Diagnostics go to stderr and bad arguments or unreadable files return a nonzero documented exit.
- `fixtures/rust-ported.json` lists every toolu host hook entry as required for the Rust conformance runner. The OpenCode adapter still imports `@toolu/core/gates` during its coexistence window.

## Failure modes and edge cases

- With no native binary, the launcher runs the matching shipped Bun bundle and prints an install advisory. With neither runtime available, an enforcing event exits 2 and a context event reports a `systemMessage` with both install commands.
- An invalid `TOOLU_BIN` does not search another native binary or fall back. A native enforcing hook crash or noncontract exit maps to exit 2. `TOOLU_RUNTIME=bun` with a missing Bun executable or bundle follows the same enforcing/context failure split.
- SessionStart advice appears once per session and remains actionable when the agent command shell cannot reach the installed binary. A disabled startup hook still preserves its title/runtime contract.
- Native pre/post hooks retain their existing fail-closed behavior for unreadable input, the shared gate state, and applicable external registry modules. A missing Bun bridge for a selected `.js` module retains the engine's documented policy.
- Debug helpers accept empty input, cap long input, preserve the named JSON fields, and surface a read error rather than reporting an empty result. Unknown stack or test output uses capped raw text.

## Acceptance criteria

- **AC-1:** With an installed binary, each of the eight toolu hook entries launches the native handler and no toolu hook process needs Bun for its own entry.
- **AC-2:** A real denied Bash command is blocked, an edited file's quality failure is recorded, and a commit is gated in actual Claude Code and Codex sessions on macOS.
- **AC-3:** Without the binary, each shipped hook entry runs its matching Bun fallback with an install advisory; `TOOLU_RUNTIME=bun` selects that fallback even when the binary exists; an invalid `TOOLU_BIN` and a missing fallback preserve the documented failure behavior.
- **AC-4:** The required Rust conformance suites and native CLI integration tests cover every migrated toolu entry, including `check-binary`, without changing OpenCode's TypeScript gate behavior.
- **AC-5:** `toolu hook pre-tools` and `post-tools` satisfy the resource budgets measured by the hook benchmark on CI.
- **AC-6:** The debug skill's real log, stack, and Bun/Cargo failure transcripts produce byte/line counts or bounded native summaries and valid JSON through `toolu debug io|log|stack|testfail`; bad paths fail visibly.
- **AC-7:** `plugins/toolu` agent-facing Markdown names native `toolu` commands, the generated CLI reference and docs describe the transition, and the Markdown–CLI drift gate passes.
- **AC-8:** `bun run test` runs both existing Bun product suites, the full Rust gate and real OpenCode acceptance pass, and remaining plugins use their own Bun bundles.

## Acceptance evidence

| AC | Real input and expected result | Boundary or failure case | Runnable check |
|---|---|---|---|
| AC-1 | Existing `hooks.json` matchers and fixture hook payloads launch native `toolu`; startup reports `runtime: native` | A hook command with the wrong name or timeout is rejected | `cargo xtask check-hooks`; `cargo test -p toolu-cli --test launcher --test tool_hooks` |
| AC-2 | Temporary installed profiles and a real macOS repository exercise denied Bash, edited code with a quality violation, and commit gate | A clean Bash/Edit path remains allowed | Actual Claude Code and Codex session transcripts plus gate-state files |
| AC-3 | Real `sh -c` launcher, installed binary, and committed fallback bundles choose the requested runtime | Invalid `TOOLU_BIN`, absent Bun, absent bundle, context vs enforcing event | `cargo test -p toolu-cli --test launcher` |
| AC-4 | Shared conformance fixtures replay each listed toolu entry against the built binary | An absent built binary or missing required entry fails the runner | `bun run test:rust-conformance`; native `crates/cli/tests` |
| AC-5 | Real hook payloads from `benchmarks/cases/hooks/` run under `cargo xtask measure` | A ported entry exceeds its declared Linux budget | `bun run bench:hooks --assert` in Linux CI; macOS report |
| AC-6 | Captured `big.log`, `stacktrace.txt`, `rust-panic.txt`, `bun-testfail.txt`, `cargo-testfail.txt` yield input counts, named summaries, and JSON | Missing file, empty input, low caps, unrecognized input | `cargo test -p toolu-cli --test debug_cli` |
| AC-7 | Updated skill and generated command tree list real native debug verbs and launcher contract | A removed Bun command in plugin Markdown fails drift check | `cargo xtask docs-cli`; `cargo xtask check-markdown-cli` |
| AC-8 | Real Bun suites and pinned OpenCode host scenarios keep passing; other plugins' own bundles still launch | A missing bundle or unresolved TypeScript import fails the gate | `bun run test`; `bun run test:opencode`; `cargo xtask gate` |

## Documentation impact

Update `docs/runtime.md`, `docs/registry.md`, `docs/install.md`, `plugins/toolu/README.md`, `plugins/toolu/skills/debug/SKILL.md`, and `AGENTS.md`; regenerate `docs/cli/`. Keep the transition notice and rollback command explicit. Remove stale Bun script references from toolu skill/command/agent prose while retaining examples where Bun is the tested project's package runner.

## Open Questions

None. The issue's temporary fallback overrides its general bundle-deletion goal for the eight live hook entries until #440. Publication of the prerequisite notice release is a merge-sequencing condition owned by the epic orchestrator, not an implementation choice.
