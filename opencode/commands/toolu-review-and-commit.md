---
description: Review, fix, and commit completed work. Does NOT merge. Does NOT push.
agent: build
subtask: true
---

# Review, Fix & Commit Completed Work
Run AFTER all tasks are completed. Reviews changes on the current branch, fixes gaps, and commits. Does NOT merge. Does NOT push.

## 1. Identify scope
- Current branch: `git rev-parse --abbrev-ref HEAD`.
- Base branch `<base>`: the repository default branch (e.g. `main` — resolve via `git symbolic-ref refs/remotes/origin/HEAD`, fall back to `main`).
- Committed changes: `git log <base>..HEAD --oneline` + `git diff <base>...HEAD --stat`.
- Uncommitted changes: `git status --short` + `git diff --stat HEAD`.
- If no changes exist (committed or working-tree), STOP and report "nothing to review".
- Group changed files by package/crate.

## 2. Launch review subagents
The reviewer matches the `push-review` gate. Prefer `caveman:cavecrew-reviewer`
when the caveman plugin is installed; otherwise use the built-in `code-review`
subagent (always available — no plugin required) configured for the highest
review effort. An optional clarity pass with the `code-simplifier` agent may run
first when installed. SessionStart warns when an optional plugin is missing.

**Across packages: concurrent.** Each package runs in its own subagent stream. **Within a package: strictly sequential** — the optional `code-simplifier` clarity pass first, then the reviewer sees the simplified diff. Never invoke the two in parallel within a package: the reviewer must see the simplified code, not the pre-simplification noise.

Per package:

1. **(optional) `code-simplifier`** — if available, point it at the recently modified files in the package. It rewrites for clarity/consistency without changing behavior (duplicated logic collapsed, verbose constructs simplified, unnecessary abstractions removed). Apply its rewrites directly to the working tree, run the package's relevant tests, then commit before invoking the reviewer.
2. **Reviewer** — the `caveman:cavecrew-reviewer` agent when the caveman plugin is installed (one-line, severity-tagged, caveman-compressed findings); otherwise the `code-review` subagent at the highest available effort. Point it at the (now simplified) changed files in the package's scope. It covers gaps, missing error handling at boundaries, untested paths, dead code, over-engineering, unclear naming, and tests that mock instead of using real data.

Every finding must include file path and line number — no vague feedback accepted. If a package has no changed files, skip its review.

## 3. Fix all reported issues
- Apply every reviewer finding. Re-run the relevant tests after each batch.
- Fix any pre-existing errors or warnings in touched files (zero tolerance).
- Same approach failed twice? STOP — change hypothesis, don't retry harder.

## 4. Run full quality gates
- Run the project's check/lint/typecheck command (whatever the project defines — package script, Makefile target, `cargo clippy`, etc.) — ZERO errors, ZERO warnings.
- Run the project's full test suite — must be green.
- If the project defines no such commands, note that in the report and continue.
- If any gate fails, fix and re-run ALL gates until fully green. No exceptions.

## 5. Commit
- Stage intentionally: `git add` only files that belong in this commit — no drive-by staging.
- Use conventional commit messages: `feat:`, `fix:`, `refactor:`, `test:`, `chore:`, `docs:`.
- One logical change per commit — split into multiple commits if scope requires it.
- Never use `--no-verify`. If hooks fail, fix the underlying issue and retry.

## 6. Completion
- Report: what was reviewed, what was fixed, final gate status, commit hashes produced.
- Do not mark complete unless all gates are green AND working tree is clean.
