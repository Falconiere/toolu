# code-review digest — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Claude (epic worker, Falconiere/toolu#861)   **Topic:** `toolu code-review digest` summarizes branch and local changes by file and item

## Problem
Agents read what a branch changed by paging through `git diff` output (epic #853: 27% of tool turns only read or search). One call that lists each changed file with its line counts, the items the changes sit in, and a few bounded hunks lets the next read be a range. It makes no model call and needs no service.

## Non-Goals
1. Moving the shaping code to a core crate (issue non-goal).
2. Old-side outlines for removed items (issue non-goal); removed names come from a keyword pattern.
3. `+N -M` counts for skipped (lock, generated, vendored, binary) files: the issue lists them as skipped only.
4. Directory or glob `--path` values: `--path` matches exact paths, as `code-review run --path` does.
5. Changing `toolu code-review run`.

## Architecture
- **Diff:** `shape::review(env, root, since, state_root)` (`crates/code-review/src/shape/snapshot.rs:14`) — the same branch-plus-local snapshot `code-review run` uses: untracked files included, toolu state excluded, noise classified, merge conflicts refused. Its `requested_base` is `--since`; `resolve_base` already takes the merge-base of that ref and `HEAD`, so `--since HEAD` gives uncommitted changes only. The default base is the push-review base (`PUSH_REVIEW_BASE`, else the branch base).
- **`--path`:** `pipeline::scope` (`crates/code-review/src/pipeline.rs:152`, made `pub(crate)`) narrows reviewed coverage exactly as `run` does. Digest prints only files in `coverage.reviewed`, and filters `coverage.skipped` and `coverage.unreviewed` to the selected paths itself (dropping `outside requested paths` entries), because `scope` does not touch them. Jev (choice, 0.73) preferred reusing `scope` over a separate filter.
- **Counts:** from each `ShapedFile.lines`: `+` = new-side lines, `-` = old-side lines.
- **Items:** one `toolu_engine::outline::outline(env, top, paths)` call over every printed file whose new side exists and whose language `outline::supported`. The snapshot tree equals the working tree, so the worktree files are the new side. For each line in `review::window::centers(file)` (`crates/code-review/src/review/window.rs:41`, made `pub(crate)`: every added line plus each hunk's new start, which marks a pure deletion), `outline::enclosing` gives the innermost item; items are deduplicated and ordered by start. An item whose every line is an added line is marked `(new)`.
- **Removed names:** `DEFINITION` (`crates/code-review/src/review/context.rs:21`, made `pub(crate)`) over old-side changed lines; names still present among the file's outline items are dropped (a changed signature is not a removal).
- **Hunks:** the zero-context `ShapedFile.patch` split at `@@` headers; each hunk prints its header and its first 8 changed lines with their `+`/`-` prefix. A longer hunk ends with `... <n> more lines: read <side> <a>-<b>` naming the lines not shown on each side, from the header's start and count (zero-context hunks list removals before additions).
- **Caps:** 80 files, 10 items per file, 3 hunks per file, 8 lines per hunk, 6 KB (6,144 bytes) of hunk text. Hunks are added file by file while the budget lasts; once a file's hunks would pass it, that file and every later one print counts and items only, and a final hint says so. Past 80 files the rest are counted.
- **Modules:** `crates/code-review/src/digest.rs` (gather, data types, caps) and `crates/code-review/src/digest/render.rs` (text). `--json` serializes the same capped data. `cli.rs` adds the verb; `lib.rs` dispatches it.

## Issue decisions
- Diff shaping reused (`review`, noise classification, `scope`, `centers`, `DEFINITION`): Architecture, Diff/`--path`/Items/Removed names.
- Item names from `toolu_engine::outline` on the new side, `(new)` for wholly added items: Architecture, Items.
- `--since <ref>` = merge-base of ref and HEAD, default branch base, uncommitted and untracked included, merge conflict refused: Architecture, Diff; Failure modes.
- Caps 80 files / 10 items / 3 hunks / 8 lines / 6 KB, then counts and items only with a `--path` hint: Architecture, Caps. Reading of "past them": past the 6 KB hunk budget, later files print counts and items only; past 80 files, the remaining files are counted in one `... <n> more files; pass --path to narrow` line, since printing them would undo the file cap.
- Without ast-grep: counts and hunks with `items unavailable`: Failure modes.
- Interface `toolu code-review digest [--since <ref>] [--path <path>]... [--json]`: Interfaces.
- Files `digest.rs`, `digest/render.rs`, `cli.rs`, `lib.rs`, git-better `SKILL.md`, `docs/cli/`: Architecture, Documentation impact.
- Deviation: none beyond the `--path` filtering of skipped and unread lists, which `scope` does not narrow (stated in Architecture).

## Interfaces / Schema
CLI: `toolu code-review digest [--since <ref>] [--path <path>]... [--json]` (global `--json`).

Text:
```
digest: main (merge-base 1a2b3c4) + uncommitted, 3 files, +12 -3
src/a.rs +5 -1 (modified)
  items: add 1-5 (new), Unit 7-7
  removed: old_fn
  @@ -3 +3,2 @@
  -  x
  +  x + 1
  +  y
src/b.py +26 -0 (added)
  items: shown 1-26 (new)
  @@ -0,0 +1,26 @@
  +line 1
  (lines 2-7 elided in this example only)
  +line 8
  ... 18 more lines: read new 9-26
skipped: Cargo.lock (lockfile)
unread: big.json (diff exceeds one whole-file evidence chunk)
```
- Header: base ref, short merge-base, `+ uncommitted` when local changes are included, printed file count, total `+`/`-`.
- A renamed file prints `<path> (from <old_path>) +N -M (renamed)`; the kind is one of `added`, `modified`, `deleted`, `renamed`, `other`.
- `items:` is `name start-end`, then `, +<n> more` past 10. Absent when the file has no items. Without ast-grep (or when the outline fails) every file prints `items unavailable` and the header line is followed by `note: items unavailable: <reason>`.
- `removed:` only when non-empty. `... <n> more hunks` past 3.
- Budget hint: `hunks omitted for <n> files past 6 KB; pass --path to see them`. File cap: `... <n> more files; pass --path to narrow`.
- No changes: `No changes against <base>`, exit 0.

JSON (one document):
```json
{"base":"main","merge_base":"<sha>","uncommitted":true,
 "files":[{"path":"src/a.rs","old_path":null,"kind":"modified","added":5,"removed":1,
   "items":[{"name":"add","start":1,"end":5,"new":true}],"items_omitted":0,
   "removed_items":["old_fn"],
   "hunks":[{"header":"@@ -3 +3,2 @@","lines":["-  x","+  x + 1"],"more":null}],"hunks_omitted":0}],
 "files_omitted":0,"hunks_budget_files":0,"items_unavailable":null,
 "skipped":[{"path":"Cargo.lock","reason":"lockfile"}],"unread":[{"path":"big.json","reason":"..."}]}
```
`more` is the `... <n> more lines: read …` text when a hunk is cut. With no changes: `{"base":"main","files":[],...}` with empty lists.

## Failure modes and edge cases
- No changes against the base (clean branch, `--since HEAD` on a clean tree, or `--path` matching nothing): `No changes against <base>`, exit 0.
- Merge conflict in the index: `code-review digest: resolve merge conflicts before reviewing`, exit 1 (from `shape::review`).
- Unknown `--since` ref: `code-review digest: base not found: <ref>`, exit 1. No merge-base: `code-review digest: no merge base with <ref>`, exit 1. Other Git failures: `code-review digest: <git message>`, exit 1.
- Not in a Git repository: `code-review digest: not inside a Git repository`, exit 1.
- No ast-grep on PATH: counts and hunks print; items show `items unavailable`; exit 0.
- Deleted file: counts, removed names and hunks; no outline.
- Unsupported language (Markdown, TOML): counts and hunks, no `items:` line.
- Oversized or unreadable patch: listed under `unread:` with the shaper's reason.
- Renamed file: printed as `new (from old)` with its counts.
- Pure deletion inside a function: the function is named through the hunk's new start (`centers`).
- Code-review disabled or no `TYPESAFE_API_KEY`: digest still runs (no model call).

## Acceptance criteria
Issue AC 1 → AC-1, AC 2 → AC-2, AC 3 → AC-3, AC 4 → AC-4, AC 5 → AC-5.

- **AC-1:** A temp repository with one commit on a branch, one uncommitted edit, one untracked file and a `Cargo.lock` change prints each file's `+N -M`, its changed item names with ranges (a new function marked `(new)`), and `skipped: Cargo.lock (lockfile)`.
- **AC-2:** In that repository `--since HEAD` shows only the uncommitted and untracked files; `--path <file>` prints only that file (no skipped `Cargo.lock`); on a clean branch with no commits past its base the output is `No changes against <base>`.
- **AC-3:** A 26-line added hunk renders 8 lines and `... 18 more lines: read new 9-26`; a fourth hunk becomes `... 1 more hunks`; the 6 KB budget switches later files to counts and items with the hint.
- **AC-4:** `toolu code-review run` behaves as before: the `toolu-code-review` test suite passes unchanged.
- **AC-5:** `docs/cli/` and the CLI contract snapshot include `digest`; the git-better skill tells agents to read why from the commits and what from the digest; `cargo xtask docs-cli && cargo xtask check-markdown-cli` pass.

## Acceptance evidence
- **AC-1, AC-2:** `crates/cli/tests/code_review_digest.rs` builds a real Git repo (`git init`, base commit on `main`, branch with one commit, then an uncommitted edit, an untracked `.rs` file and a `Cargo.lock` edit), runs the built `toolu code-review digest` with the real ast-grep on PATH, and asserts stdout lines; also `--json` parses as one document. Boundary: `--since HEAD`, `--path`, clean branch. Check: `cargo test -p toolu-cli --test code_review_digest`.
- **AC-3:** unit tests in `crates/code-review/src/digest/tests/render_test.rs` over real `ShapedFile` patches produced by Git in a temp repo (26-line added hunk, four hunks, many files). Check: `cargo test -p toolu-code-review digest`.
- **AC-4:** Check: `cargo test -p toolu-code-review`.
- **AC-5:** Check: `cargo xtask docs-cli && cargo xtask check-markdown-cli && cargo test -p toolu-cli --test contract`.

## Documentation impact
`plugins/git-better/skills/git-better/SKILL.md` (“Reading a branch”: why from commits, what from `toolu code-review digest`), `docs/cli/code-review.md` and `docs/cli/commands.json` regenerated by `cargo xtask docs-cli`, `crates/cli/tests/fixtures/commands_json.snap`.

## Open Questions
None. `--path` reuse and skipped-file counts were decided with Jev and evidence above.

## Spec review
- Round 1: Jev coverage 0.49, buildable 0.90. Fixed: issue-decision mapping with the reading of "past them", rename line format, concrete hunk example.
- Round 2: Jev coverage 0.75, buildable 0.90. Checklist: problem, non-goals, one decided architecture, failure exits, real-data evidence per AC, docs impact, no open questions — no blockers.

**Status:** Approved
