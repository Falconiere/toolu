# Conformance harness Rust seam — Design

**Date:** 2026-10-05   **Status:** Approved   **Author:** Codex   **Topic:** Issue #409, select a Rust CLI command for existing black-box hook cases

## Problem

The Bun conformance harness and some tests run committed `hooks/dist/*.js` entries directly. Rust ports need the same fixtures and expectations without copying roughly 200 test files. A selected but missing Rust executable must be an immediate, intelligible failure.

## Non-Goals

1. Implementing a Rust hook, the final Rust CLI contract, or OpenCode's future native shim.
2. Replacing TypeScript unit tests of imported epic-orchestrator functions before #434/#435 deliver the resident engine.
3. Changing bundle packaging and launcher tests that inspect bundle paths rather than execute an entry.

## Architecture

`@toolu/conformance/harness/entry-command` is the single resolver. Callers identify a plugin and entry and supply the existing bundle path and, when necessary, its existing launcher argv. With `TOOLU_IMPL` unset, it returns the existing Bun command unchanged. Selected toolu-plugin hooks use `<TOOLU_RUST_BIN_DIR or repo target/release>/toolu hook <entry>`; `toolu/pre-tools` therefore runs `toolu hook pre-tools`. Other plugins use `toolu <plugin> hook <entry>`, preserving identity where names such as `session-start` collide. Harness hook runners, executable plugin tests, and the protected-dispatch conformance helper use it. The shared subprocess runner keeps current stdin, environment, timeout and output capture.

`fixtures/rust-ported.json` is the single CI port list. A Rust conformance matrix job reads it; an empty list exits successfully without requiring a CLI binary. A nonempty list builds the release binary and runs `test:unit` plus `test:conformance` with an exact selector derived from that list, so ported entries face the existing black-box suites and the CLI matrix.

The current `test:conformance` matrix has four suites. `protected-files` and `spaces-cwd` call `protected-dispatch` and execute `toolu/pre-tools`, so they take the Rust route when selected. `bootstrap-readiness` checks OpenCode's diagnosis of a missing startup bundle; `surface-drift` checks generated TypeScript files. They remain active in the matrix and retain their specific checks in both modes. `spawn-check` is a helper, not a fifth suite.

The issue's epic-script alternative is used. `report.test.ts`'s persisted status, history and usage scenarios are assigned to #435's Rust worker-report black-box tests. `epic-watch.test.ts`'s event, lock, checkpoint, timing and acknowledge scenarios are assigned to #434's engine state-machine and #435's action black-box tests. Those TypeScript suites keep running for the current TypeScript implementation. Epic #402 intentionally replaces the watcher with a resident engine, so no legacy `toolu epic watch` argv is specified here.

## Interfaces / Schema

```ts
type EntryCommand = {
  plugin: string;
  entry: string;
  bundle: string;
  defaultArgv?: string[];
};
type ResolvedCommand = { argv: string[]; implementation: "bun" | "rust" };
resolveEntryCommand(entry: EntryCommand, env?: NodeJS.ProcessEnv): ResolvedCommand;
```

- Selector grammar: absent or empty means Bun; `rust` selects every hook entry; `rust:<plugin>/<entry>,...` selects exact entries. Whitespace, duplicate entries, empty items, unknown syntax and path separators inside a component are errors rather than silent fallback.
- The binary is `toolu` in `TOOLU_RUST_BIN_DIR` if set, else `<repository>/target/release/toolu`. A relative override is resolved from the test process's current working directory. The selected path must be a regular executable file. The error names the selector entry and expected path before a subprocess starts.
- The Rust command receives the same stdin, cwd and environment as the Bun command. Selector and binary-directory variables are harness controls: the harness `run` (which drops every `TOOLU_*` key) and `protected-dispatch` strip them from the hook's environment. A test that spawns with its own `process.env` copy passes them through, which neither implementation reads.
- `fixtures/rust-ported.json` is `{ "entries": [] }`; values use the exact `<plugin>/<entry>` selector form. CI rejects malformed or duplicate values.

## Failure modes and edge cases

Unselected entries continue to use their supplied Bun command even if `TOOLU_RUST_BIN_DIR` names a missing directory. An invalid selector fails during resolution, never falling back to Bun. A selected missing, non-file or non-executable binary fails at setup with its resolved path. Concurrent suites do not mutate `process.env`; the resolver accepts an explicit environment for isolated tests. A Rust subprocess nonzero exit stays a normal captured test result, labelled with its implementation so a parity failure identifies Rust. Paths with spaces are argv elements, not shell interpolation.

## Acceptance criteria

- **AC-1:** With `TOOLU_IMPL` absent, the pre-tool golden corpus and `test:conformance` have their existing outputs and outcomes.
- **AC-2:** With `TOOLU_IMPL=rust:toolu/pre-tools` and an executable stub that echoes allow, every golden pre-tool case reaches that executable with `hook pre-tools` and its original stdin. Allow cases pass; deny cases fail their existing expectation and name the Rust implementation.
- **AC-3:** Selecting a nonexistent Rust executable fails before spawn and identifies both `toolu/pre-tools` and the exact expected binary path. Invalid selectors fail explicitly; unselected entries still run Bun.
- **AC-4:** Executable hook targets in existing black-box tests and the `protected-dispatch` helper use the resolver; `test:conformance`'s protected-files and spaces-cwd cases reach a selected Rust stub, while the other two matrix checks still run.
- **AC-5:** The CI Rust conformance leg reads the sole `fixtures/rust-ported.json` list; an empty list is a successful no-op and a nonempty list builds the release binary, then runs `test:unit` and `test:conformance` with the exact selector.
- **AC-6:** The #434/#435 replacement inventory names the `report.test.ts` and `epic-watch.test.ts` behaviors, leaving no unassigned direct epic script suite.

## Acceptance evidence

| AC | Real input and expected observation | Boundary and runnable check |
|---|---|---|
| 1 | Existing committed bundles and golden fixture corpus produce unchanged decisions; four CLI matrix suites pass | `bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-golden.test.ts`; `bun run test:conformance` with selector unset |
| 2 | A real executable shell stub records argv and stdin and returns a Claude allow JSON; one allow case passes and the deny case fails with `rust:toolu/pre-tools` in diagnostics | Isolated sandbox test under `tools/toolu-conformance/src/harness/__tests__/` and a golden-suite run using `TOOLU_RUST_BIN_DIR`; no simulated resolver result |
| 3 | Missing absolute target path appears in setup error; unselected sample bundle still runs | Resolver/subprocess tests using a nonexistent temp directory and an actual sample bundle |
| 4 | Protected `.env` fixture in real temporary directories reaches the stub for both normal and spaced cwd; bootstrap and surface suites report their existing pass/fail outcomes | `bun run test:conformance` with selected stub, plus the black-box executable-target audit |
| 5 | Empty committed manifest prints no-op; a populated temporary manifest attempts a real Cargo build and propagates suite failures | CI runner tests and workflow job inspection; CI job runs the committed empty manifest |
| 6 | Existing script tests are mapped by scenario to #434/#435 without claiming their legacy argv is a Rust API | Spec inventory review and `bun test plugins/epic-orchestrator/scripts/__tests__/report.test.ts plugins/epic-orchestrator/scripts/__tests__/epic-watch.test.ts` |

## Documentation impact

Update `docs/testing.md` with selector syntax, binary lookup, port-list workflow and the distinction between hook-dispatch and non-hook conformance suites. The epic-script replacement inventory stays in this spec.

## Open Questions

None. #434/#435 own the replacement Rust engine behavior and CLI verbs; that contract does not block this hook seam.

## Execution decisions

- **The grep criterion.** The issue asks that `git grep 'hooks/dist/'` in test files show only the resolver. Some test files have the bundle path itself as their subject: packaging and drift checks, the launcher, `hooks.json` and skill text, golden command text, and the OpenCode bootstrap, whose production code spawns bundles. `bundle-references.test.ts` enforces the criterion with a reasoned allowlist of those files, and fails on a stale entry. Every other mention goes through `bundlePath`, `entryArgv` or `launchedArgv`. Jev preferred this to rewriting text fixtures or leaving the criterion unchecked (0.95).
- **Launcher runs.** Many tests run a hook through its `hooks.json` launcher (`sh -c`) rather than `bun <bundle>`. `launchedArgv` keeps that default and switches to the Rust command when selected. The same guard rejects direct `launcherCommand(` calls in tests, except the launcher's own tests and the two wiring tests that compare `hooks.json` text.
- **Rust labels.** The pre-tools case suites append `implementationTag("toolu", "pre-tools")` to their test names, so a failing case under `TOOLU_IMPL=rust:toolu/pre-tools` reads `… [rust:toolu/pre-tools]`. `protected-dispatch` puts the same label in its failure message.
- **CI group.** The `rust-conformance` job is gated on a new `ports` path group: the port list, its runner, the conformance harness, `plugins/**`, `packages/**` and the Rust paths. The job installs the toolchain only when the list is non-empty. `CARGO` overrides the cargo executable, as cargo's own convention does, so the runner's build-failure path is tested with a real executable.
