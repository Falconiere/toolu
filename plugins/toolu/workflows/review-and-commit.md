# Review, fix, and commit completed work

Run this only after implementation is complete. Review the current branch, fix
real findings, and commit. Do not merge or push.

## 1. Establish scope

- Resolve the current branch with `git rev-parse --abbrev-ref HEAD`.
- Resolve the default base through `refs/remotes/origin/HEAD`, falling back to
  `main` only when the remote default is unavailable.
- Inspect committed scope with `git log <base>..HEAD --oneline` and
  `git diff <base>...HEAD --stat`.
- Inspect uncommitted scope with `git status --short` and
  `git diff --stat HEAD`.
- Stop with `nothing to review` if both scopes are empty. Otherwise group
  changed files by package or crate.

## 2. Review the diff

Use the host mapping in `workflows/host-mapping.md`. Run one correctness review
over the requested committed scope; passes are sequential so every reviewer
sees the latest diff and writes state for that scope.

Discover `ponytail-review` in the host skill catalog. If installed and Ponytail
is not disabled by the user, invoke it first for the requested scope, including
uncommitted changes. It is a full read-only quality review; verify its findings.
Skip a repeat only when it already reviewed the same unchanged scope. If absent
or disabled, continue without installing it.

Prefer `code-review:review` for the committed diff. Follow its automatic
`toolu-review:review` fallback when native review cannot complete, preserving
scope and real findings. If the native skill is unavailable, invoke the fallback
directly. Failed or unavailable fallback blocks completion without approval.
Review uncommitted changes directly against the checklist below; native review
covers committed changes only. Optional clarity/simplification passes run before
correctness review, never concurrently with it.

Review correctness, security, boundary error handling, regressions, missing
tests, dead code, unnecessary abstraction, unclear naming, and tests that mock
the integration instead of using real data. Every finding must cite a file and
line; discard vague feedback.

## 3. Fix findings

- Verify each finding against the code before changing it.
- Apply every valid finding and rerun relevant tests after each batch.
- Fix existing errors or warnings in touched files.
- If the same approach fails twice, stop and change the hypothesis.

## 4. Run full gates

Run the project's full check, lint, typecheck, and test commands as defined by
repository instructions and toolchain configuration. If none exist, say so.
Fix failures and rerun all required gates until green.

## 5. Commit intentionally

Stage only files belonging to each logical commit. Use scoped Conventional
Commit messages (`type(scope): description`) and split unrelated changes. When
git-better is installed, end each message with an `Intent:` trailer saying why
the change is needed. Never use `--no-verify`; fix hook failures instead.

After the final commit, run `code-review:review` (with the same automatic
fallback) on the final committed diff. Fix findings, rerun gates and commit
before re-reviewing until complete and clean.

Report reviewed scope, fixes, final gate results, and produced commit hashes.
Completion requires green gates and a clean working tree.
