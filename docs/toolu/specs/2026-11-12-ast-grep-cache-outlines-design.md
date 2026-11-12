# Content-addressed outlines and repository map — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Codex   **Topic:** Cache outlines and summarize large directories

## Problem
Repeated outline scans waste ast-grep work across linked worktrees, and the 150-file directory limit hides most of a large repository.

## Non-Goals
1. Cache other ast-grep commands or quality scans.
2. Change small Git directory maps or the public `outline(env, cwd, paths)` interface.

## Architecture
Keep cache lookup inside `toolu_engine::outline::outline`. `toolu_state::content_id::blob_ids` obtains working-tree OIDs from one `git hash-object --no-filters --stdin-paths` process. Store path-free file outlines under `common_dir/toolu/outline/v1-<ast-grep version>/<language>/<blob oid>.json` with `write_atomic`. Scan only misses through #857's batched runner. Before publishing misses, rehash the paths once as a batch and write only entries whose OIDs still match; a real scan-time edit proved that the initial hash alone can poison the shared cache. Missing/corrupt entries and cache I/O failures scan normally. Outside Git, scan without persistence. SessionStart sweeps entries older than 14 days then oldest entries beyond 20,000, without failing startup.

For at most 150 supported files, preserve #857's map. Above 150, enumerate every supported file; summarize one row per containing directory with file count, total lines, a module doc from `lib.rs`, `mod.rs`, `index.ts`, or `__init__.py`, and top public names. Module doc is the first nonempty leading `//!`, `/**`, or Python module docstring sentence. Sort directory rows by path; when more than 148 directories exist, print the first 148 and an omitted-directory count after the header. Keep text bounded by 150 lines and 12 KiB. Outside Git, enumerate supported files by walking the requested directory; inside Git, retain `git ls-files` and ignore semantics.

## Interfaces / Schema
`blob_ids(env: &Env, cwd: &Path, paths: &[String]) -> Option<Vec<String>>`; a failed hash process gives no cache keys. `outline` retains its existing signature. The cache JSON contains `lines` and `items` only, never a path. CLI and JSON for small directories remain compatible; large directory JSON has directory summary rows.

## Failure modes and edge cases
An unreadable, truncated, or schema-invalid cache entry is a miss and is replaced after a successful scan. An unwritable cache is ignored. A Git hash failure or path containing a line break scans without caching. A file changed during scanning is returned for that call but is not cached under its old OID. Identical content in linked worktrees shares an entry; dirty and untracked content gets its actual working-tree OID. The cache version includes schema and the installed ast-grep version. An ast-grep scan failure still fails the command; a single oversized file keeps #857's `too large to outline` behavior. Empty directories produce an empty map. Symlinks are not followed by the outside-Git walk. Sweep skips unreadable metadata and warns instead of failing startup.

## Acceptance criteria
- **AC-1:** Two outlines of the same file in a real Git repository scan once; editing it scans again, and a second linked worktree reuses the entry for identical content.
- **AC-2:** A truncated cache entry is treated as a miss and rewritten with valid, path-free JSON.
- **AC-3:** A SessionStart sweep removes entries older than 14 days, then the oldest entries beyond 20,000.
- **AC-4:** Outlining a 160-file Git repository spread over a few directories prints rows covering all files in at most 150 lines, including count, lines, module doc, and public names; more than 148 directories produces an omission count.
- **AC-5:** An outside-Git directory outlines supported files without writing a cache; small Git maps retain their existing output.

## Acceptance evidence
- AC-1: `cargo test -p toolu-cli --test ast_grep_outline`; a logging wrapper around real ast-grep, real dirty file, `git worktree add`, and an edit during the real scan prove scan count, shared cache, and the no-poison fallback.
- AC-2: `cargo test -p toolu-engine outline::cache`; a real Git repository with a truncated entry proves fallback and rewritten JSON.
- AC-3: `cargo test -p toolu-ast-grep retention`; real files with pinned mtimes prove age and capacity order.
- AC-4: `cargo test -p toolu-cli --test ast_grep_outline`; 160 actual source files across directories prove coverage and line bound.
- AC-5: `cargo test -p toolu-cli --test ast_grep_outline`; a temporary non-Git directory and the existing #857 fixture prove both paths.

## Documentation impact
Update `docs/ast-grep/README.md` for cache location, retention, large map and outside-Git behavior. Regenerate CLI docs only if help text changes.

## Open Questions
None. The issue and merged #857 settle the public interface and cache location; the brainstorm records the two remaining map defaults.
