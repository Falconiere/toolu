# Code review diff shaping — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Codex   **Topic:** Shape committed review evidence for #662

## Problem

The native review needs a bounded, accurate view of the same committed diff as the push gate. A wrong base, invisible skipped path, or context line treated as citable can approve a defect or attach a finding to the wrong line.

## Non-Goals

1. Model calls, finding validation, gate-state writes, and the user-facing run command belong to later issues.
2. Working-tree changes are never folded into the committed review diff.

## Architecture

`shape` uses `toolu_state::git::base_branch` and `toolu_state::diff_sha::diff_sha`, then asks Git for a merge-base from the repository top-level. It reads NUL-delimited `--name-status -M` and the exact gate `--name-only` path set. Batched committed attributes and Git grep results classify generated headers; a truncated grep batch is bisected, while other batch failures fall back to one path at a time. Each status entry gets its own zero-context Git patch with literal paths, no text conversion, and merge-base attributes, selected by both paths for a rename, so quoted header paths never have to be decoded to identify a file. A changed line has its actual old/new line and side, including a committed CRLF line's carriage return; context lines are not citable. Every removed line, including one in a replacement hunk or deleted file, is citable as `side: old` at its old line number. Whole-file entries are grouped into bounded chunks. A rename records both paths. If the gate lists both rename paths because `diff.renames=false`, both paths share the one shaped file's coverage category. Deleted files use the merge-base blob.

The existing `toolu review` path helper keeps Git's quoted `--name-only` spelling, because the push gate compares `reviewed_files` with those lines. Shape coverage holds raw UTF-8 paths, so #669 must map a non-ASCII path to the gate's spelling before it writes gate state.

## Interfaces / Schema

`shape(env: &Env, root: &Path, requested_base: Option<&str>) -> Result<Shaped, ShapeError>`. `Shaped` carries `base_ref`, `merge_base`, `diff_sha`, `gate_files`, `files`, `chunks`, `coverage`, `renames`, and `uncommitted`. A file entry carries `old_path`, `path`, `kind`, and `lines: Vec<ChangedLine>`. `ChangedLine` has `side: Side`, `line`, and `text`. `ShapeError` distinguishes absent base/merge-base, empty diff, and Git/metadata failure. A path only receives a passing coverage category if its data is fully shaped.

## Failure modes and edge cases

A non-rename path with no changed-line anchors, including an added empty file, is unreviewed; a pure rename may have zero anchors. A gate path absent from rename-aware metadata is unreviewed, while an unexpected extra metadata path fails the shape run.

An absent or shallow base history returns an explicit error, never a fallback base. Other merge-base failures remain Git errors. Empty committed diff returns `EmptyDiff`. Failure to enumerate the gate path set or a malformed/truncated metadata response returns a `ShapeError`, never a partial pass. Per-file patch or blob failure marks that path unreviewed. Any gate path not accounted for by `-M` metadata is unreviewed with a mismatch reason; no extra metadata path is silently approved. Binary, lockfile, vendored, root build-output, and generated paths are skipped with a reason. A rename into a noisy path is reviewed when its old path was source. Generated detection uses directory names, `linguist-generated` attributes committed at the merge-base (so the reviewed branch cannot mark its own changes generated), and generator notices at the start of the first five source lines. Added-file markers are read from HEAD and deleted-file markers from the base blob; a modified or renamed file is generated only when both its merge-base and HEAD sides are. The branch's own attributes cannot turn source into a binary diff. An oversized single file is unreviewed; no line is silently omitted. The gate hash and merge-base are checked again after shaping to reject a moving diff. Uncommitted changes set a flag for later gate-state handling.

The issue's requested noise policy allows a new file under a noise path or bearing a generator notice to be skipped from branch-controlled evidence. The skip is reported in coverage; a later policy change would be needed to require review of those additions. When a long matching line overflows batched generator detection, the batch is bisected and a one-path retry reads only the first 8192 committed blob bytes, which are sufficient for the five-line header rule.

Per-file patches start at the resolved merge-base commit. Git 2.42 or newer is required for `diff --attr-source`; older Git leaves affected files unreviewed with a Git error. A file-to-directory replacement or typechange that yields multiple patch sections is unreviewed, even when one section is binary. Per-file patches pin short submodule output and zero inter-hunk context, so user diff configuration cannot drop or add hunk lines. Non-UTF-8 path metadata fails the whole shape run because coverage paths are UTF-8 strings. These cases stay fail-closed.

## Acceptance criteria

- **AC-1:** Given a branch with a common ancestor, shaping uses the gate's base and hash; a shallow or unrelated base reports an error, and an empty diff is distinct.
- **AC-2:** Given a `git mv` plus an edit, shaping yields one rename with `old_path → path` and real new/old changed-line anchors.
- **AC-3:** Given lockfile, vendored, build, generated (attribute and marker), and binary changes, every gate path appears once in skipped coverage with a reason and contributes zero review lines.
- **AC-4:** Given a deleted text file, shaping reads its merge-base content and exposes old-side removal anchors.
- **AC-5:** Given a mixed diff, every exact gate path belongs to exactly one coverage category; only changed lines are citable, and oversized files are visibly unreviewed.

## Acceptance evidence

- AC-1: Temporary real Git repositories with diverged history, unrelated history, and an empty branch; `cargo test -p toolu-code-review shape` checks base/hash or typed error.
- AC-2: A real Git repository with `git mv` and a committed line edit; the same focused test checks one entry, rename map, and line numbers.
- AC-3: A real Git repository with the named noise files, committed `.gitattributes`, and a binary blob; the focused test checks every skip and zero review lines.
- AC-4: A real Git repository deleting a text file; the focused test checks base text and old-side lines.
- AC-5: Real mixed repository fixture plus an oversized diff; the focused test compares the coverage union to `git diff <base>...HEAD --name-only` and checks chunk and unreviewed behavior.

## Documentation impact

The design and plan document this internal interface. No CLI help or user-facing command changes in this issue.

## Open Questions

None. #668 owns finding validation against these anchors; #669 owns gate-state use of `uncommitted`.
