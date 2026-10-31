# Code-review caller search cache — Design

**Date:** 2026-10-31   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** Cache `git grep HEAD` reference hits by definition name for one review and compile the definition regex once (#762, epic #737)

## Problem
`review::source::packages` calls `context::callers` once per changed file. Each enclosing definition name runs `references`, a `git grep` of `HEAD`, with no memory of names already searched for another file, and `enclosing` recompiles the definition regex per file. A review whose files share an enclosing name (`new`, `fmt`, `run`) repeats identical Git subprocesses.

## Non-Goals
1. No change to what is sent: the >20-hit omission, 64 KiB read cap, per-file exclusions, context byte budget, stable order and skip notes stay as they are.
2. No cross-review or on-disk cache; the cache lives for one `packages` call.
3. No worktree search; only committed `HEAD` is searched.
4. No CLI, config or docs surface change.

## Architecture
A private `Callers` struct in `crates/code-review/src/review/context.rs` is created once at the top of `packages` and passed by `&mut` to each file's lookup. It owns:
- the definition `Regex`, compiled once (`Result` kept; on failure each file still gets the existing "caller context unavailable" note and no context);
- `hits: BTreeMap<String, Option<Vec<Hit>>>`, the **raw** `git grep` outcome by name, where `None` means over `CONTEXT_HITS_MAX`, truncated, timed out, spawn failure or a Git exit other than 0/1, and `Some(vec![])` means no match;
- `searches: usize`, the number of `git grep` subprocesses run, for tests.

The cache stores unfiltered hits. Each file then applies its own changed-line and own-definition-line exclusions to a clone/borrow of the raw hits, then the byte budget, so a filtered result is never reused for another file. Over-limit and failed names push their skip note for each file that asks, exactly as today. Cache lifetime is the `packages` call, which runs against one repository and one `HEAD`; a new review builds a new `Callers`, so no stale data crosses a HEAD or repository change.

## Interfaces / Schema
```rust
pub(super) struct Callers { pattern: Result<Regex, String>, hits: BTreeMap<String, Option<Vec<Hit>>>, pub(super) searches: usize }
impl Callers { pub(super) fn new() -> Self; pub(super) fn of(&mut self, (&Env, &Path), &Shaped, (&ShapedFile, &str), cap: usize, notes: &mut Vec<String>) -> Option<String> }
fn enclosing(pattern: &Regex, file: &ShapedFile, text: &str) -> BTreeMap<String, u32>
```
`packages` keeps its signature. Output text, notes and order are byte-identical to today's.

## Failure modes and edge cases
- Name with exactly 20 hits: kept; 21: omitted with the existing note, searched once.
- No match (Git exit 1): cached as empty, no context, no note.
- Git failure/timeout/truncation (output over 64 KiB): cached as `None`, skip note per asking file; not retried within the review.
- Paths containing `:` and binary files: `-z` NUL parsing and `-I` are unchanged.
- Uncommitted worktree edits are never seen (`HEAD` only).
- Regex compile failure: note per file, empty result, as today.
- Context budget exhausted for a file: its skip note is unchanged; the cached hits remain usable by later files.

## Acceptance criteria
- **AC-1:** In a committed repo where one definition name encloses changes in several files, `git grep` runs once for that name in the review, and each file's context shows its own distinct callers.
- **AC-2:** The definition pattern is compiled once per review (one `Callers`), and the search stays on committed `HEAD`.
- **AC-3:** Exactly 20 hits are kept and 21 are omitted with the existing skip note, a no-match name yields no context, a failed/unreadable search yields the skip note, and over-cap output and context-budget exhaustion keep their notes, all with outcomes cached.
- **AC-4:** A file's changed-line and own-definition-line exclusions are not leaked to another file sharing the name.
- **AC-5:** Paths holding `:`, binary files and uncommitted worktree changes behave as before.
- **AC-6:** A second review of the same repo after a new commit sees the new `HEAD` hits (no stale cache), and the Git subprocess count and release timing for repeated enclosing names are measured before and after.

## Acceptance evidence
- AC-1: temp Git repo with `fn run` in three files plus a caller file; `Callers.searches == 1`, per-file context strings asserted. Check: `cargo test -p toolu-code-review context`.
- AC-2: same test asserts `searches`; pattern compiled in `Callers::new` (no `Regex::new` in `enclosing`, checked by ast-grep).
- AC-3: temp repos with 20 and 21 references, no references, an over-64 KiB grep output, and `context_bytes` tiny; compare notes with the existing `source_test` expectations. Check: `cargo test -p toolu-code-review`.
- AC-4: two files share name `run`; a caller line changed in file A is present in file B's context and absent from A's.
- AC-5: existing `src/odd:name.rs` test stays green; add an uncommitted-edit assertion and a binary file hit.
- AC-6: commit a new caller, build a new `Callers`, assert it appears; before/after timing and `searches` count on a release build with many files sharing one name, recorded in the PR.

## Documentation impact
None — internal performance change with identical output; no README, `docs/` or CLI surface describes the search.

## Open Questions
- Should the cache be shared across `packages` calls (e.g. recheck)? Owner: worker. Decision: no, per-review only. Non-blocking.
