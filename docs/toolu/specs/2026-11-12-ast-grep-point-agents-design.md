# Point agents at outline, symbol and digest — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** epic worker (#862)   **Topic:** nudges, savings kinds and docs that steer agents to the new read commands

## Problem
`toolu ast-grep outline`, `toolu ast-grep symbol` (#860, #868) and `toolu code-review digest` (#861, #883) shipped, but agents still hand-roll definition listings with `grep -n "pub fn "` and look up definitions with `rg "fn name"`, then read whole files. Nothing points them at the new commands, and `toolu ast-grep savings` cannot show whether they are used.

## Non-Goals
1. A nudge on whole-file reads of large files (issue non-goal).
2. Outline/symbol advisories on the dedicated Grep tool. Decided Bash grep/rg only: the issue's examples and checks are Bash commands, and the existing "a declaration name stays on Grep" contract (fixture `bats: Grep for a function name is silent`) stays. Jev choice: bash 0.87.
3. Savings kinds for `symbol`, `map` or other toolu subcommands (issue names outline and digest only).

## Architecture
- `crates/ast-grep/src/nudge.rs`: `Kind` gains `Outline` and `Symbol`. `shell()` keeps its search-argument collection; after the structural check, it classifies the searched arguments (skipping flags) with a new `declaration(arg)` helper. The helper strips regex syntax (`\s \b \w \d`, `^ $ ( ) ? * + [ ] { } . | \`) into spaces and splits words. Only declaration keywords and modifiers → `Outline`. A declaration keyword followed by an identifier that is not itself a keyword or modifier → `Symbol`. Precedence: structural > outline > symbol > generic. A search whose arguments name a non-code path or glob (`NON_CODE_PATH`, `NON_CODE_GLOB`) never classifies as outline or symbol.
- `message()`: `(Outline, Available)` and `(Symbol, Available)` return the issue's texts. For `Missing` and `OptOut` they return the existing `BashGeneric` text for that state, so users without ast-grep see no change.
- `crates/ast-grep/src/savings.rs`: `kind()` for Bash/Shell returns `"outline"` when the command runs program `toolu` with argv `ast-grep outline`, `"digest"` for `code-review digest`, else the existing `ast-grep` detection. Reuses `program_indexes`/`program_at` and `toolu_shell::analyze`.
- `report::render` already groups any kind; no change.
- Docs: one Behaviour bullet in `plugins/toolu/hooks/docs/session-start.md`; one Sync sentence in `plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`.

Keywords: `fn struct enum trait type const class interface def function func`. Modifiers: `pub crate super export default async unsafe extern static`.

## Interfaces / Schema
- Advisory texts (exact):
  - Outline: ``Listing definitions? `toolu ast-grep outline <path>` prints every item's signature, first doc line and line range in one call; then read only the range you need.``
  - Symbol: ``Looking for a definition? `toolu ast-grep symbol <name>` prints it with every reference and its enclosing item.``
- Ledger lines (existing shape): `{"kind":"outline","returned":N,"full":0}` and `{"kind":"digest","returned":N,"full":0}`.
- Session bullet: ``- Outline before reading: `toolu ast-grep outline <path>`, `toolu ast-grep symbol <name>`, `toolu code-review digest`.``
- Worker brief Sync sentence: ``Map code with `toolu ast-grep outline <path>` and `toolu ast-grep symbol <name>` before opening whole files; after a resume, `toolu code-review digest` shows what the branch already changed.``

## Failure modes and edge cases
- Grep in a pipe (`cat a.rs | grep "pub fn main"`): still silent; classification only sees search commands at pipeline start or under xargs.
- Structural plus declaration (`grep "impl Foo"`): structural STOP wins.
- `rg TODO docs/`, `grep -rn TODO src`: generic text unchanged.
- `rg "fn x" docs/`: non-code path, generic text.
- ast-grep missing / opted out: outline and symbol cases fall back to the generic Bash text.
- Unparsed shell (`analysis.unknown`): silent, as today.
- `toolu ast-grep outline` with an empty response: no ledger line (existing `returned_bytes` rule). Unwritable ledger: best effort, tool allowed (existing).
- `toolu ast-grep symbol`, `toolu ast-grep map`: no ledger line (not a recorded kind).

## Acceptance criteria
- **AC-1:** With ast-grep available, `grep -n "pub fn \|pub(crate) fn " crates/x/src/a.rs` yields the outline advisory; `rg "fn hunk_start"` yields the symbol advisory; `rg TODO docs/` yields neither (generic text).
- **AC-2:** With ast-grep missing or opted out, those outline/symbol commands yield the existing generic Bash text for that state.
- **AC-3:** A post-tool Bash call running `toolu ast-grep outline <path>` and one running `toolu code-review digest` each append one ledger record (`outline`, `digest`), and `toolu ast-grep savings <ledger>` lists both kinds.
- **AC-4:** The session-start doc stays within its 110-word budget and every command it names exists; the worker-brief text tests and markdown CLI check pass.

## Acceptance evidence
- AC-1: unit `cargo test -p toolu-ast-grep nudge` on `toolu_shell::analyze` of the three exact commands plus `message` texts; real-binary fixture cases added to `fixtures/ast-grep/nudge.json` with `deviation.contains`, run by `cargo test -p toolu-cli --test ast_grep_hook`. Boundary: piped grep and `rg "fn x" docs/` stay non-outline.
- AC-2: fixture cases with `state: missing` / `opt-out` for the outline command expect the generic text, via the same CLI test.
- AC-3: unit `cargo test -p toolu-ast-grep savings` on `kind()`; real-binary test in `crates/cli/tests/ast_grep_hook.rs` that posts both payloads through `toolu hook post-tools`, reads the ledger, and runs `toolu ast-grep savings`. Boundary: `toolu ast-grep symbol x` records nothing.
- AC-4: `cargo xtask gate --only context-budget`, `cargo xtask check-markdown-cli`, `cargo test -p toolu-epic-orchestrator launch::brief`, `cargo nextest run -p toolu-cli --test epic_markdown`.

## Documentation impact
`plugins/toolu/hooks/docs/session-start.md` and the worker-brief reference are the user-facing surfaces and change. No CLI surface changes, so `docs/cli/` and the contract snapshot stay.

## Open Questions
None. Grep-tool scope decided (Non-Goal 2).

## Spec review
- Acceptance evidence: 🔵 consider: Jev rated "every AC has a real-input runnable check" low (0.11). Verified by hand: AC-1–AC-3 each run the real `toolu` binary through `toolu hook pre-tools`/`post-tools` and `toolu ast-grep savings` (CLI test), with unit tests on the real shell parser; AC-4 runs the repository's own gate checks on the real files. No mock stands in. Kept.
- Scope: AC-2 follows from "used only when ast-grep is available" (Jev 0.9); the non-code path rule backs the `rg TODO docs/` criterion (Jev 0.81).
- Open questions: none.
