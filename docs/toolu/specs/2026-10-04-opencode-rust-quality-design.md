# OpenCode Rust post-edit quality — Design

**Date:** 2026-10-04 **Status:** Approved **Author:** Cursor agent **Topic:** OP-20 native rust-quality enforcement

## Problem

OpenCode registers the rust-quality post-tool module, but registration alone does not prove that native edits run its rules. An invalid Rust edit must reach the model and leave a failing quality gate that refuses later commit and push attempts. Nothing in the repository exercises the shipped rust-quality bundle through the OpenCode bridge or the pinned host.

## Non-Goals

1. Add `cargo check`, `clippy` or `rustfmt` runs. rust-quality is static on every host: its toolchain is the `Cargo.toml` marker and `cargo` prerequisite plus real `ast-grep` Rust scans (error handling, mock imports and definitions) and in-process line rules.
2. Change the shared OpenCode post-tool bridge, the core dispatcher or the rust-quality rules without a reproduced defect.
3. Change rust-quality's linked-worktree behavior. Like python-quality, it checks linked worktrees on Claude Code and Codex, and OpenCode keeps that.
4. Change rust-quality's path rules. Error-handling, no-mocks and docs rules key on `/src/` and `/tests/` in the edited path as given, exactly as on other hosts; a relative `src/x.rs` therefore skips the error-handling, inline-test and docs checks, while size, suppression, `unsafe` and test-location rules still run.
5. Treat a post-tool diagnostic as undoing a completed edit.

## Architecture

Reuse the OP-06 `tool.execute.after` bridge and its per-path patch walk, as the python-quality port (#353, PR #389) did. A quality violation records the failing gate entry and returns an advisory, so every completed patch path is checked and the diagnostics merge. The rust-quality SessionStart bundle publishes `rust-quality@toolu__rust-quality.js`. Its module requires `Cargo.toml` at the git toplevel and `cargo` on `PATH`, then calls `fileQuality` on `.rs` paths with `skipLinkedWorktrees: false`.

A probe adapter test with the committed bundle in real git projects is expected to pass for writes, edits, multi-file patches with move and delete, selection, prerequisites, project isolation and the linked worktree; if it does, no production change is needed. This issue adds the missing evidence:

- `tools/toolu-opencode/src/adapter/__tests__/rust-quality-post.test.ts`, a real-bundle adapter test that runs real `git`, `cargo` detection and `ast-grep` in sandboxed git projects.
- `tooling/src/opencode-host/scenarios-rust-quality-smoke.ts`, `tooling/src/opencode-rust-quality-smoke.ts` and a `smoke:opencode-rust-quality` script, reusing `quality-smoke-shared.ts`.
- Documentation of the tested OpenCode behavior and prerequisites.

The decisive trade-off is evidence over new code: the shared bridge is already generic, so a Rust-specific path would add surface without a demonstrated defect. Jev chose keeping rust-quality static over adding a compiler run (confidence 1.0) and keeping the linked-worktree check (confidence 1.0).

## Interfaces / Schema

- Selection: the existing `.opencode/toolu/plugins.json` `enabled` list (`["toolu", "rust-quality"]`) and `selectedPluginSpecs` passed to `dispatchPostTool`. No new configuration.
- Native tools: OpenCode's pinned `tool.execute.after` receives `write`, `edit` and `apply_patch` completed args and a mutable text result; the bridge appends a bounded `QUALITY VIOLATION` diagnostic.
- Gate: `.opencode/tmp/quality-gate-status.json` under the project's git toplevel. A file entry keyed by the edited path as given (absolute or relative), `source: "rust-quality-hook"`, `status: "failing"` on a violation; a clean edit, deletion or move source clears it.
- Script: `bun run smoke:opencode-rust-quality [scenario-id]` exits 0 when every scenario passes on the pinned OpenCode 1.18.34 CLI.

## Failure modes and edge cases

- rust-quality not selected, no `Cargo.toml` at the toplevel, or no `cargo` on `PATH`: no rust-quality diagnostic and no gate file.
- A non-`.rs` file (`notes.md` containing `#[allow(dead_code)]` or `.unwrap()`): not checked, no entry.
- Multi-file patch: each added or updated `.rs` destination is checked once. A deletion and the source side of a move clear their prior entries; a moved destination is checked. Two violating destinations both stay failing and both appear in the appended diagnostic.
- `ast-grep` absent: the error-handling and no-mocks rules are skipped, as on other hosts; a broken scan is itself a violation. The tests assert `cargo` and `ast-grep` are installed so a missing tool cannot make them pass vacuously.
- Linked worktree: checks run, and the gate file belongs to the linked worktree's toplevel, not the main checkout.
- Two independent projects keep separate gate files; clearing one does not clear the other.
- A thrown host tool error has no after callback; an interrupted bash result cannot clear failure state (OP-06 behavior, unchanged).

## Acceptance criteria

- **AC-1:** With rust-quality selected in a real Cargo project, native OpenCode write and edit calls run the shipped rules on `.rs` files; a `#[allow(dead_code)]` suppression produces a visible "Forbidden lint suppression" diagnostic and a failing per-file entry with `source: "rust-quality-hook"`, and a clean edit returns the gate to passing.
- **AC-2:** A native multi-file patch that moves a failing `src/old.rs` (`.unwrap()`) to a still-invalid `src/moved.rs` (`.expect()`), deletes another failing `src/removed.rs`, adds `tests/added.rs` importing `mockall` and adds `src/notes.md` leaves exactly the moved destination and the added test failing, shows both diagnostics (both found by real ast-grep Rust scans), and creates no entry for the `.md`.
- **AC-3:** After a violating Rust edit, native commit and push attempts are refused before their marker side effects.
- **AC-4:** With rust-quality disabled, without `Cargo.toml`, or without `cargo` on `PATH`, the same invalid file gets no rust-quality diagnostic or gate file. Two independent projects keep distinct gate state, a linked worktree is checked against its own gate file, and existing Claude Code and Codex rust-quality tests stay green.
- **AC-5:** The pinned OpenCode 1.18.34 host loads the selected module in an isolated profile and demonstrates real tool bytes, model-visible diagnostics and gate state for the edit, patch and disabled scenarios; the repository quality gate passes.

## Acceptance evidence

| AC | Real input and expected result | Boundary and runnable check |
| --- | --- | --- |
| AC-1 | Git project with `Cargo.toml`, real `cargo`; write `src/bad.rs` with `#[allow(dead_code)]`, then edit it clean; write `notes.md` with the same text. Inspect result text and gate file. | `bun test tools/toolu-opencode/src/adapter/__tests__/rust-quality-post.test.ts`; `bun run smoke:opencode-rust-quality rsquality.edit`. |
| AC-2 | Seed failing `src/old.rs` and `src/removed.rs`, then one patch: move old to `src/moved.rs`, delete removed, add `tests/added.rs` with `use mockall::predicate;`, add `src/notes.md`. Inspect files, gate keys and diagnostic. | `bun test …/rust-quality-post.test.ts`; `bun run smoke:opencode-rust-quality rsquality.patch`. |
| AC-3 | After the invalid write, `git commit` / `git push` (marker commands in the smoke); markers stay absent and both bash calls error with the quality-gate reason. | `bun test …/rust-quality-post.test.ts`; `bun run smoke:opencode-rust-quality rsquality.edit`. |
| AC-4 | Repeat the invalid write unselected, with no `Cargo.toml`, and with a `PATH` holding only `git` and `ast-grep`; a second project; a linked worktree with committed `Cargo.toml`. | `bun test …/rust-quality-post.test.ts plugins/rust-quality/hooks/src/__tests__`; `bun run smoke:opencode-rust-quality rsquality.disabled`. |
| AC-5 | Pinned CLI, exact SDK, isolated profile and scripted loopback provider run the three scenarios. | `bun run smoke:opencode-rust-quality`, `bun run check:opencode-host`, `bun run test`. |

## Documentation impact

Update `plugins/rust-quality/README.md` (an OpenCode section), `docs/rust-quality/README.md`, `docs/opencode.md` (a "Rust post-edit quality" section beside the Python one) and `docs/opencode-host-contract.md` (name the new smoke). Regenerate the OpenCode resource mirrors those docs feed.

## Open Questions

None blocking. Whether the pinned-host smoke reaches tool execution on this machine is reported with its own result, separately from the adapter evidence.

## Spec review

**Status:** Approved. Issue criterion 1 (native edits, patches, move/delete, real toolchain) maps to AC-1, AC-2 and AC-5; criterion 2 (visible diagnostic, failing gate, commit/push refused) to AC-1 and AC-3; criterion 3 (disabled plugin, unrelated files, no mocked toolchain) to AC-4 and AC-2. Every AC names a real input, an observable result and a runnable check; the tests assert `cargo` and `ast-grep` exist so a missing tool cannot pass vacuously. The no-production-change architecture is conditional on the probe adapter test passing against the committed bundle; a failure returns this spec for revision. Jev scored the coverage 1.97 of 2 (confidence 0.95).
