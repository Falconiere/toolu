# OpenCode TypeScript post-edit quality — Design

**Date:** 2026-10-03 **Status:** Approved **Author:** Codex **Topic:** OP-18 native ts-quality enforcement

## Problem

OpenCode can register the ts-quality post-tool module, but registration alone does not prove that native edits run its rules. An invalid TypeScript edit must reach the model and leave a failing quality gate that refuses later commit and push attempts.

## Non-Goals

1. Add a TypeScript compiler run after every edit; ts-quality's existing contract is per-file rules with project and tool prerequisites.
2. Change the shared OpenCode post-tool bridge or other language plugins without a reproduced defect.
3. Treat a post-tool diagnostic as undoing a completed edit.

## Architecture

Reuse the OP-06 `tool.execute.after` bridge, its normalized edit records and selected registry dispatch. Resolve relative edit paths against the native call's project directory; keep the host's git project root as the quality-state root. A real patch test exposed that the shared dispatcher stops after the first post block, leaving later completed edits unchecked. Add an opt-in `dispatchPostTool` option for OpenCode that walks every completed patch record and combines blocking reasons; retain the existing default for Claude and Codex. The ts-quality SessionStart bundle publishes `ts-quality@toolu__ts-quality.js`; the existing module detects a git-tracked tsconfig and available package manager before calling `fileQuality` on `.ts` and `.tsx` paths. For OpenCode `.tsx` files, select `tsx` instead of `ts` as the real ast-grep inline-rule language; preserve the existing rule strings on Claude and Codex. The existing per-file quality gate remains the source of truth, and the core pre-tool gate refuses commit and push while it is failing.

Add issue-owned subprocess and pinned-host scenarios using isolated real git projects and the committed plugin bundle. Exercise native write/edit and multi-file patch calls, including a move and a deletion. Assert changed bytes, model-visible diagnostics, per-file gate entries, recovery, selected-plugin isolation and later commit/push refusal. Fix only behavior these checks demonstrate to be wrong. Preserve the existing Claude and Codex module behavior.

## Interfaces / Schema

- Selection uses the existing `.opencode/toolu/plugins.json` `enabled` list and `selectedPluginSpecs` passed to `dispatchPostTool`; no new configuration is introduced.
- OpenCode's pinned `tool.execute.after` receives completed tool args and a mutable text result. The bridge maps `write`, `edit` and `apply_patch` to core post events, appending a bounded diagnostic to the result. The after hook cannot undo the tool.
- `dispatchPostTool(..., { continuePostBlocks: true })` is used only by the OpenCode bridge. It keeps walking normalized patch records after a per-file post block and returns a single block with all blocking reasons. The omitted/default option retains the prior short-circuit behavior.
- ts-quality passes inline ast-grep rules with `language: tsx` for OpenCode `.tsx` paths and keeps `language: ts` for `.ts` and existing hosts. No new tool or parser is introduced.
- The existing `quality-gate-status.json` stores a file entry keyed by edited path and `source: "ts-quality-hook"`. A violation sets `status: "failing"`; a corrected file or removed source clears that module's entry.

## Failure modes and edge cases

- A disabled ts-quality plugin, a non-TypeScript file, an untracked/missing tsconfig, or an unavailable package manager does not invoke ts-quality checks or create its gate entry. The core post bridge may still run for other selected modules.
- Preserve ts-quality's existing linked-worktree skip: an edit there leaves its quality entry untouched. An independent git project owns its own root and gate file; relative paths are resolved within that root.
- A valid multi-file patch checks each added/updated destination once. A deletion and the old side of a move clear their prior entries; the moved destination is checked if it is a TypeScript file. An unrelated extension does not acquire a ts-quality entry.
- Two invalid destinations in one completed patch both keep failing entries and both appear in the appended diagnostic, regardless of patch record order. A runtime failure still propagates instead of being converted to a clean result.
- A failed or cancelled tool has no confirmed successful edit check. A thrown host tool error has no after callback; an interrupted after result must leave existing failure state intact.
- A rule or subprocess failure appears in the result as a post-check diagnostic and must not be reported as a clean pass. A failure already stored in the gate remains until the normal clean edit or removal path clears it.
- Test execution uses the real Bun package manager, installed `ast-grep`, committed bundles and native OpenCode host. The loopback model only scripts host tool calls; it does not replace the toolchain or post dispatcher.

## Acceptance criteria

- **AC-1:** With ts-quality selected in a real TypeScript project, native OpenCode write and edit calls run the shipped rules on `.ts` and `.tsx` files; a `console.log` or structural empty catch produces a visible diagnostic and a failing per-file gate entry.
- **AC-2:** A native multi-file patch checks each changed TypeScript destination once, clears stale entries for a deleted path and the source of a move, and does not create an entry for an unrelated extension.
- **AC-3:** After a violating edit, native commit and push attempts are refused before their marker side effects; a corrected edit clears the failure, while an interrupted result cannot clear it.
- **AC-4:** With ts-quality disabled, without its TypeScript project/package-manager prerequisites, or on a linked worktree, the same invalid file does not run ts-quality or create its gate entry. Two ordinary projects keep distinct gate files, and existing Claude and Codex behavior remains green.
- **AC-5:** The pinned OpenCode 1.18.34 host loads the selected module in an isolated profile and demonstrates real tool bytes, model-visible diagnostics and gate state; the repository quality gate passes.

## Acceptance evidence

| AC | Real input and expected result | Boundary and runnable check |
| --- | --- | --- |
| AC-1 | Git project with tracked `tsconfig.json`, `bun.lock`, actual `ast-grep`; write/edit `.ts` and `.tsx` containing `console.log` and `catch {}`. Inspect the returned text and gate file. | `bun test tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts`; `bun run smoke:opencode-ts-quality`. |
| AC-2 | Patch adds a bad `.ts`, moves a previously failing `.tsx`, deletes another failing `.ts`, and changes a `.md`. Inspect remaining files and gate entries. | `bun test tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts`; `bun run smoke:opencode-ts-quality`. |
| AC-3 | Follow an invalid write with `git commit` and `git push` marker commands; marker files stay absent. Correct the file, then send an interrupted result and a completed clean edit. | `bun test tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts`; `bun run smoke:opencode-ts-quality`. |
| AC-4 | Repeat the invalid write with ts-quality unselected, no tracked tsconfig, no lock file, and a linked worktree. Verify no ts-quality diagnostic or entry; verify a separate project has independent state and run existing Claude/Codex golden cases. | `bun test tools/toolu-opencode/src/adapter/__tests__/ts-quality-post.test.ts plugins/ts-quality/hooks/src/__tests__`. |
| AC-5 | Pinned CLI, exact SDK, isolated OpenCode profile and scripted loopback provider execute the native edit/patch flow. Inspect tool states, output text and persisted gate. | `bun run smoke:opencode-ts-quality`, `bun run check:opencode-host`, `bun run test`. |

## Documentation impact

Update `plugins/ts-quality/README.md`, `docs/ts-quality/README.md`, `docs/opencode.md`, and `docs/opencode-host-contract.md` with the tested OpenCode behavior and prerequisites. Synchronize generated OpenCode resource mirrors when those source docs change.

## Open Questions

None blocking. The existing OP-06 live smoke timed out before the first provider request on one worker; a new pinned-host attempt will report its own result separately from subprocess evidence.

## Spec review

**Status:** Approved. The acceptance evidence names real files, native tool results, persisted state and runnable checks for every AC. The linked-worktree and project-root boundary was added after reviewing the host binding and ts-quality module. The amendment identifies the opt-in dispatcher interface, aggregate result and unchanged default. Jev judged the amendment uncertain (0.35); direct inspection of `dispatchRecords` found the stop, and actual `ast-grep run` matched the moved `.tsx` fixture's empty catch. The real bundle and default-behavior checks are the approval boundary.
