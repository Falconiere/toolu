# Working-tree content hash without touching the index — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** epic worker (Falconiere/toolu#873)   **Topic:** `tree_sha` beside `diff_sha`

## Problem
Detached ledger / journal work (epic Falconiere/toolu#869) needs one hash of the
working tree's current content so a save can skip when nothing changed. Today's
`diff_sha` only hashes the committed branch diff (`git diff base...HEAD`), so an
edit that is not committed is invisible. Callers need a helper that never writes
the real index or any working-tree file.

## Non-Goals
1. No callers in this change. The detached ledger run (sibling issues) is first.
2. No change to `diff_sha`, push-review, or other git helpers' public contracts.
3. No hashing of ignored paths; no optional flags or config.
4. No CLI surface.

## Architecture
Add `crates/core/state/src/tree_sha.rs`, exported from `lib.rs`, mirroring
`diff_sha`'s shape (`Env` + `Path` → `Option<String>`, `None` on failure).

Sequence (issue decision; Jev chose `read_tree_add_a` / `system_tempfile`,
confidence ≥0.91):

1. Create a temporary index path outside the repo (`tempfile`; already a
   `toolu-state` dependency).
2. Run git under `env.clone().with("GIT_INDEX_FILE", <temp>)` (process clears
   env when `Spec.env` is set, same as `diff_sha`).
3. `git read-tree HEAD` → `git add -A` → `git write-tree`.
4. Drop/delete the temp index. Never pass the real `.git/index` path.

`git add -A` with that temp index stages tracked changes and untracked
non-ignored files; ignored paths stay out. Keep a private `git` runner inside
`tree_sha.rs` (copy of `diff_sha`'s local helper). Do not extract a shared
module in this change.

## Interfaces / Schema
```rust
/// The working-tree content hash, or `None` when git fails or the path is not a repository.
pub fn tree_sha(env: &Env, repo_root: &Path) -> Option<String>
```
Module: `crates/core/state/src/tree_sha.rs`. Crate export: `pub mod tree_sha;`
in `crates/core/state/src/lib.rs`. Unit tests: `crates/core/state/src/tests/tree_sha_test.rs`,
wired with `#[cfg(test)] #[path = "tests/tree_sha_test.rs"] mod tests;`.

## Failure modes and edge cases
| Input / situation | Observable behavior |
|---|---|
| Not a git repository | `None` |
| `git` missing / empty `PATH` | `None` |
| Unborn `HEAD` / `read-tree HEAD` fails | `None` |
| Any step non-zero or truncated | `None` (same filter as `diff_sha`) |
| Temp index create fails | `None` |
| Empty committed tree, clean worktree | `Some(<empty tree id>)` when the three git steps succeed |
| Concurrent real-index writers | Still must not alter `.git/index` bytes or porcelain for this call |

## Acceptance criteria
- **AC-1:** Two `tree_sha` calls on an unchanged real git worktree return the same hash string (trimmed stdout of `git write-tree`), and that string is non-empty.
- **AC-2:** Given a repo with a tracked file committed, rewriting that file's contents (not `git add`) changes the next `tree_sha` value.
- **AC-3:** Given a clean committed repo, creating a new untracked file that is not ignored changes the next `tree_sha` value.
- **AC-4:** Given a committed `.gitignore` that ignores `*.tmp`, creating `scratch.tmp` does not change `tree_sha`.
- **AC-5:** Immediately before and after one successful `tree_sha` call on a dirty worktree (tracked edit + untracked file present), `git status --porcelain` text and the byte contents of `<repo>/.git/index` are equal.
- **AC-6:** `tree_sha` returns `None` for a tempfile directory that is not a git repo, and returns `None` for a valid repo when `Env` has `PATH` set to `""`.

## Acceptance evidence
All evidence uses real `git` in a `tempfile::tempdir` repository. No mocks.
Fixture builder (shared by AC-1..AC-5): `git init -q -b main`; write `tracked.txt`
and `.gitignore` with `*.tmp`; `git add` both; commit with
`git -c user.name=t -c user.email=t@t commit -q` (same pattern as
`crates/core/state/src/tests/diff_sha_test.rs`). Env for success paths:
`Env::from_pairs([("PATH", std::env::var("PATH").unwrap())])`.

| AC | Test name (new) | Real input | Expected observable | Boundary |
|---|---|---|---|---|
| AC-1 | `unchanged_tree_is_stable` | clean fixture; `tree_sha` twice | both `Some(h)` with `!h.is_empty()` and equal | clean stability |
| AC-2 | `tracked_edit_changes_hash` | clean fixture; hash; overwrite `tracked.txt`; hash | hashes differ | tracked content |
| AC-3 | `untracked_file_changes_hash` | clean fixture; hash; create `extra.txt`; hash | hashes differ | untracked inclusion |
| AC-4 | `ignored_file_does_not_change_hash` | clean fixture; hash; create `scratch.tmp`; hash | hashes equal | ignore exclusion |
| AC-5 | `real_index_and_porcelain_unchanged` | dirty fixture (edit `tracked.txt` + untracked `extra.txt`); capture `git status --porcelain` via `Command` and `fs::read(.git/index)`; call `tree_sha`; capture again | porcelain strings equal; index byte vectors equal; `tree_sha` is `Some` | index isolation |
| AC-6 | `missing_repo_or_git_returns_none` | (a) empty tempfile dir; (b) valid fixture with `PATH=""` | both calls return `None` | fail closed |

Runnable check for every row: `cargo test -p toolu-state --lib -- tree_sha`.

## Documentation impact
None — internal library helper; no CLI, skill, or user-facing docs. Crate `//!`
on `tree_sha.rs` documents behavior for agents reading the source. Update
`lib.rs` module list comment to mention `tree_sha` beside `diff_sha`.

## Open Questions
None blocking. Settled from issue #873 + epic #869 + Jev:

- Seed/stage: `read-tree HEAD` then `add -A` then `write-tree` (issue wording).
- Temp index: system tempfile, not under `.git/`.
- Fail closed to `None` when any git step fails, matching `diff_sha`.
- No callers in this PR.
