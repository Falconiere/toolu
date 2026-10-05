# Language-neutral parity fixtures — Design

**Date:** 2026-10-05   **Status:** Approved   **Author:** Codex   **Topic:** Shared JSON contracts for TypeScript and Rust tests

## Problem
The Rust port cannot read parity inputs embedded in TypeScript case tables. #408 requires committed JSON under `fixtures/` to be the single source for both implementations, preserving the existing test assertions and case counts.

## Non-Goals
1. Change any production hook, rule, parser, or assertion semantics.
2. Port the consumers to Rust in this issue; dependent issues do that.
3. Move `tooling/fixtures/{conventions,guardrails,pr-babysit-herdr-smoke}` before #439. Those overlays test TypeScript tooling and have no named Rust parity consumer yet.

## Architecture
Move existing JSON contract trees for shell, config, portable-core, gate-coverage, and Codex hook schemas to `fixtures/<area>/`, updating every reader and CI path mapping. Export the remaining named case families as committed JSON and make the current TypeScript suites load them. Keep runtime harnesses in TypeScript: they translate JSON setup operations and path tokens into real sandbox files, git actions, and hook payloads. The records, including names, inputs, setup parameters, expectations, and golden outputs, live only in JSON. Bounded interpreters replace case-specific closures. A fixture audit compares the inventory and case counts and rejects orphaned captures and residual TypeScript case tables.

For shell parsing, record the pinned unbash parser's result for each distinct input in `fixtures/shell/unbash-baseline.json`. This is a recorded differential oracle for #416, never regenerated during tests. Existing golden JSON keeps its observed output and base commit unchanged while moving to the shared tree.

## Interfaces / Schema
The new case records live at `fixtures/gates/{lifecycle,pre-tool-modules-a,pre-tool-modules-b,pre-tool-modules-c,pretool-corpus,posttool-corpus}.json`, `fixtures/quality/{ts,python,rust,runner}.json`, `fixtures/ast-grep/{nudge,savings,report}.json`, `fixtures/host/root.json`, `fixtures/state/cases.json`, `fixtures/statusline/cases.json`, and `fixtures/opencode/{permission-evaluate,lifecycle-events}.json`. The existing golden captures move beside the matching cases. The moved trees are `fixtures/{shell,config,portable-core,gate-coverage,codex-hook-schemas}`.

Each case-family file has `{ "version": 1, "cases": [...] }`. Every case has a unique `name`, its current serializable fields, and optional `setup` as an ordered array of `{ "op": string, ...arguments }`. Dynamic fixture inputs use a tagged object such as `{ "kind": "bash", "command": "..." }` or `{ "kind": "tool", "toolName": "...", "toolInput": {} }`. Path tokens `$PROJECT`, `$ROOT`, `$HOME`, and `$HOST_STATE` are expanded only by the harness after it creates a fresh sandbox. Setup operations are limited to the real actions used by these suites: file write/remove/copy, git command or commit, config/registry installation, and executable stub creation. An unknown operation or token is an error. Family-specific fields and operations are documented in `fixtures/README.md`; JSON stores no JavaScript source or executable expression.

The five existing JSON trees retain their current payload shapes. Golden captures retain `{ "base": string, "cases": { "<name>": <capture> } }` where that is the current format. `unbash-baseline.json` has `{ "version": 1, "parser": <pinned unbash version>, "cases": [{ "input": string, "result": <JSON parse result> }] }`, with one result per distinct shell input.

The measured baseline for the existing named arrays is lifecycle 117 (50 SessionStart, 67 UserPromptSubmit), pre-tool modules A 109, B 110, C 180, pre-tool corpus 33, post-tool corpus 16, ts-quality 120, python-quality 96, rust-quality 119, and ast-grep 79 (38 nudge, 35 savings, 6 report). A checked inventory records the remaining inline test-case counts before conversion. The acceptance audit compares names as well as counts, so replacing a case with another cannot hide a loss.

## Failure modes and edge cases
Missing or malformed JSON, duplicate names, missing or extra golden captures, unknown setup operations, unknown path tokens, and paths escaping a sandbox fail the suite before a hook runs. Empty stdin, malformed hook input, missing tools, and negative rule cases retain their existing assertions. The parser baseline records parse failures as structured error results rather than silently omitting an input. Cases run in isolated real sandboxes, so concurrent test files do not share writable fixture state.

## Acceptance criteria
- **AC-1:** The existing shell, config, portable-core, gate-coverage, and Codex hook schema readers load identical data from `fixtures/<area>/`; no reader depends on their old paths.
- **AC-2:** Lifecycle and pre-tool modules A/B/C tests load their named inputs and golden captures from JSON; adding a valid case to `fixtures/gates/pre-tool-modules-a.json` makes the TypeScript suite discover it without a TypeScript edit.
- **AC-3:** Pre-tool and post-tool conformance corpora, including edit/shell split and #283 gate cases, load JSON records and preserve their sandbox setup and decision assertions.
- **AC-4:** ts-, python-, rust-quality, ast-grep, and shared quality runner suites load JSON case records and golden outputs with their existing outcomes and counts.
- **AC-5:** Host-root, state, statusline, and OpenCode permission/evaluate and lifecycle event cases are exported to JSON for #414, #415, #431, and #462, with current TypeScript consumers loading them.
- **AC-6:** `fixtures/shell/unbash-baseline.json` has one pinned-parser result for every distinct shell fixture input, including parse failures, and a check detects missing or extra inputs.
- **AC-7:** `fixtures/README.md` documents file shapes, consumer issue numbers, the three deferred tooling-only trees, and the before/after case count of each suite; `bun run test` passes with no assertion change.

## Acceptance evidence
- **AC-1:** Existing `bats-parity.json`, config fail-closed inputs, protected-files pre-tool captures, gate inventory, and Codex output schemas; `bun run test:unit` and `bun run test:docs` exercise readers, including invalid config and schema cases.
- **AC-2:** `.env` protected-file case, malformed config case, plan-ledger and push-review cases plus their committed bash captures; `bun test plugins/toolu/hooks/src/__tests__`. A temporary valid JSON-only case addition proves discovery.
- **AC-3:** `git push`/waiver post-tool case, multi-path patch and malformed stdin; `bun run test:conformance`, with before/after case-name equality.
- **AC-4:** Representative TypeScript `as` rule, Python assignment, Rust layout rule, ast-grep nudge, and quality runner edit/delete paths; `bun test plugins/{ts-quality,python-quality,rust-quality,ast-grep}/hooks/src/__tests__ packages/toolu-core/src/quality/__tests__`, with before/after case-name equality.
- **AC-5:** Existing host-root, state, statusline, and OpenCode tests, including an invalid permission or lifecycle payload; their focused Bun suites and a JSON inventory check.
- **AC-6:** `bats-parity.json` and `issue-283.json` command inputs; a deterministic baseline completeness check and the shell test suite.
- **AC-7:** The tracked fixture inventory and suite counts before/after, `bun run test`, `bun run check:ci-paths`, and `fixtures/README.md` review.

## Documentation impact
Add `fixtures/README.md`; update `docs/testing.md`, shell fixture documentation, and path references in affected docs and CI mapping.

## Open Questions
None. The tooling-only tree boundary is set by #408's explicit option and recorded in the brainstorm.
