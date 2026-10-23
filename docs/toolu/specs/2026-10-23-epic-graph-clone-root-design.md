# Epic graph clone root — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** resolve `clone_root` from known roots, never the working directory, and clone a shared missing repo once

## Problem
`graph::build` falls back to `input.cwd` when the epic repo has no checkout and saves its parent as `clone_root`. The engine passes its state dir, the CLI the shell's cwd. A graph saved from a vanished cwd recorded `/`, and three concurrent launches raced `gh repo clone` into one path (#586, #583, #584).

## Non-Goals
1. No new config key; roots come from herdr workspaces and `EPIC_REPO_ROOTS`.
2. No change to how checkouts resolve.
3. No retry or recovery of already-uncertain leases (#583, #584).

## Architecture
Remove `cwd` from `BuildInput`. `clone_root` is the first of: parent of the epic repo's checkout; the first `EPIC_REPO_ROOTS` entry (a parent directory); the parent of the first herdr root; the parent of any other resolved checkout. `build` returns an error when none exists or the result is `/`. `clone_checkout` takes `toolu_state::lock::with_lock` on `<state_dir>/clone-<name>` (stale 120 s, timeout 130 s) and, under it, skips `gh repo clone` when `<checkout>/.git` already exists, so concurrent launches clone once.

## Interfaces / Schema
`BuildInput` loses `cwd: PathBuf`. `graph.json` `clone_root` stays a string, now never `/` or empty. New `fn resolve_clone_root(...) -> Result<PathBuf, String>` in `graph/from_tracker.rs` (or a sibling file for size).

## Failure modes and edge cases
- No checkout, no `EPIC_REPO_ROOTS`, no herdr roots: `build` fails with `cannot resolve clone_root: set EPIC_REPO_ROOTS or open the repository in herdr`; nothing is saved.
- Candidate is `/` or empty: skipped; if nothing remains, same error.
- Lock busy past timeout: closure runs unlocked (existing lock behaviour); the `.git` check still limits damage.
- Existing non-git directory at the target: clone fails with git's message, propagated.
- Dry run: logs the clone line, takes no lock.

## Acceptance criteria
- **AC-1:** Given no checkout, `EPIC_REPO_ROOTS=<dir>`, `build` records `clone_root == <dir>`, whatever the process cwd.
- **AC-2:** Given no checkout, no roots, no herdr roots, `build` returns an error naming `clone_root` and saves nothing.
- **AC-3:** Given the epic repo's checkout at `/a/b/repo`, `clone_root` is `/a/b`; a checkout at `/repo` (parent `/`) is refused.
- **AC-4:** Given two threads preparing the same missing repo, `gh repo clone` runs once and both get the same checkout path.

## Acceptance evidence
- AC-1/2/3: `graph/tests/from_tracker_test.rs` with the Jira fixture tracker and temp dirs; `cargo test -p toolu-epic-orchestrator from_tracker`.
- AC-4: `launch/tests/checkout_test.rs` with a stub `gh` executable on `PATH` that creates `<dest>/.git` and logs each call, two threads; `cargo test -p toolu-epic-orchestrator checkout`.

## Documentation impact
None — `clone_root` and `EPIC_REPO_ROOTS` are not described in user docs beyond `docs/cli/` flags, which do not change.

## Open Questions
None.
