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

## 2. Review each package

Use the host mapping in `workflows/host-mapping.md`. Independent packages may
be reviewed concurrently; within one package, passes are sequential so every
reviewer sees the latest diff.

Prefer the installed toolu reviewer (`toolu-review:review` in the active host's
invocation syntax). An installed repository-specific reviewer may be used when
it writes a compatible push-review attestation. Optional clarity/simplification
passes run before correctness review, never concurrently with it.

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

Stage only files belonging to each logical commit. Use Conventional Commit
messages and split unrelated changes. Never use `--no-verify`; fix hook
failures instead.

Report reviewed scope, fixes, final gate results, and produced commit hashes.
Completion requires green gates and a clean working tree.
