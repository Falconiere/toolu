# Shared Rust quality runner — Design

**Date:** 2026-10-09   **Status:** Approved   **Author:** Codex   **Topic:** One engine quality flow for ts, python and rust rules

## Problem

The three post-edit quality plugins repeat the same file selection, structural scan, and gate settlement. Their Rust ports need one shared engine contract so a multi-file edit is processed consistently without three scans or cloned runners.

## Non-Goals

1. Porting the language-specific checks of #426–#428.
2. Switching installed plugin hooks from Bun to Rust before those rule crates exist.
3. Changing gate schema, lock protocol, configuration limits, or TypeScript behavior.

## Architecture

`toolu_engine::quality` exposes a `QualityRule` trait and one batch runner. A caller supplies the normalized edit records of one hook event, its event/context, and enabled rules. The runner resolves paths once, clears removed paths by owner, skips missing/foreign files, honors each rule's linked-worktree policy, scans remaining files in one ast-grep subprocess, calls each selected rule's per-file check, and settles its source's gate entry through `toolu-state`. `toolu-state::edit_records` handles patch normalization upstream; `toolu-state::gate_file` owns locked writes. A single-file adapter extracts the legacy `quality-edit.ts` path and edit-field semantics for direct post-tool events and fixture parity.

Each rule supplies its language, extensions, project check, gate source/reason, linked-worktree policy, optional directories of ast-grep YAML, and a check that receives the shared scan result for its file. Rules run only when enabled and the edited extension matches. Rule YAML comes from every enabled rule crate, including an enabled crate with no matching file in this event. A rule with no structural YAML does not cause a scan. The runner reads only `.yml`/`.yaml` regular files from declared directories in stable order; invalid/unreadable YAML produces a scan failure given to the rule, not an empty result.

## Interfaces / Schema

- `EditedFile { path: String, absolute: PathBuf, removed: bool }` preserves the hook's path as gate key and uses a cwd-resolved path for filesystem checks.
- `QualityRule` exposes `language`, `extensions`, `source`, `reason`, `project_enabled`, `skip_linked_worktrees`, `ast_rule_dirs`, and `check(file, ctx, scan) -> QualityFindings`.
- `QualityFindings { errors: Vec<String>, advisories: Vec<String> }`; a violation decision is `Decision::Advisory` with `QUALITY VIOLATION — fix before proceeding:\n` followed by newline-terminated errors. Clean findings clear only the rule's own entry and join nonempty advisories with newline.
- `AstGrepScan` distinguishes `Missing`, `Ok { hits, empty }`, and `Failed { stage, exit_code, stderr_first }`. A hit has rule id, source file, one-based line, excerpt, source text, and first-line marker.
- The batch runner returns ordered per-file decisions. The caller combines them with the normal post-tool decision precedence; the runner itself never changes that precedence.

## Failure modes and edge cases

- No path, missing file, directory, or foreign extension: allow without a state write or scan.
- Delete or move source: clear its source-owned entry even if the file no longer exists; the destination is checked in the same batch.
- A repeated path in one event is settled once per owning source, using the last record's operation; paths are otherwise processed in input order.
- `CLAUDE_FILE_PATHS` wins over an edit tool's `path`, `file_path`, then `target_file`; null and false are skipped, but empty string stops fallback. Split edit fields override inherited `TOOLU_EDIT_*`; `toolu_edit_*` input is the last fallback.
- A symlink to a regular file counts as a regular file. A linked worktree is determined from git-dir/common-dir, and state is rooted in that worktree when the rule permits checks there.
- Missing ast-grep, a nonzero exit, stderr, invalid JSON, blank stdout, and malformed matches remain distinct outcomes. A rule decides the violation text for its scan outcome, preserving its present language-specific behavior. A process timeout or output truncation is a failed scan.
- Gate writes use the existing state writer's lock and fallback behavior. Warnings are returned to the caller; they are not silently discarded.

## Acceptance criteria

- **AC-1:** Every `fixtures/quality/runner.json` edit and run case produces the Rust path, decision, and gate state the TypeScript case expects.
- **AC-2:** A real patch moving `a.ts` to `a.py` clears the ts-quality source entry and records the python-quality source entry in one batch.
- **AC-3:** A real linked worktree `.py` edit records its gate entry under that worktree's state directory; a ts-quality rule configured to skip worktrees does not run there.
- **AC-4:** With enabled rules for more than one language and multiple edited files, one batch spawns no more than one ast-grep process and routes each hit to its owning file and rule.
- **AC-5:** Real ast-grep output, missing binary, stderr/nonzero exit, malformed JSON, and signal exit preserve the outcomes in `fixtures/quality/runner.json`.
- **AC-6:** `cargo xtask gate` passes with no clone, quality suppression, or layer violation from the new module.

## Acceptance evidence

| AC | Real input and observable result | Boundary/failure | Runnable check |
|---|---|---|---|
| AC-1 | Exported runner JSON, real temp Git repo and gate file; all paths, decisions and entries match | Missing and foreign paths | `cargo test -p toolu-engine quality` |
| AC-2 | Actual `apply_patch` move records; old entry absent, new source entry present | Removed source file | `cargo test -p toolu-engine quality_move` |
| AC-3 | `git worktree add` and a `.py` file; gate under linked root | ts skip policy | `cargo test -p toolu-engine quality_linked` |
| AC-4 | Two YAML rule directories and `.ts`/`.py` edits; both hits routed, one child spawn observed through an executable fixture | No selected files yields zero spawns | `cargo test -p toolu-engine quality_scan_batch` |
| AC-5 | Exported scan JSON cases plus real ast-grep; exact hit/error variants match | Missing, malformed, signal | `cargo test -p toolu-engine quality_scan` |
| AC-6 | Full workspace gate over final diff | All required gate steps | `cargo xtask gate --base origin/main --title 'feat(engine): share post-edit quality runner'` |

## Documentation impact

Update `AGENTS.md` engine key-file description and `docs/registry.md` to document the shared `QualityRule` contract and integration point for #426–#428. No CLI or installed hook documentation changes until those rule ports land.

## Open Questions

None. The linked-worktree policy follows the current language modules; the caller integration into compiled rule crates belongs to #426–#428.
