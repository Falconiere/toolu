# Commit all changes

Commit the current working tree without broad re-exploration.

1. Run this short workflow inline. The Codex `quick-task` profile is read-only;
   it cannot stage, fix or commit. Use the active host's normal command tools.
2. Run only `git status` and `git diff --stat` to establish scope. Explore more
   only to fix a reported failure.
3. Stage all changes with `git add -A`. Run the full project quality suite at
   delivery, before opening a PR or pushing review fixes.
4. Commit with a concise, scoped Conventional Commit subject
   (`type(scope): description`) of at most 72 characters.
   Group a body by package only when it adds useful context. When git-better
   is installed, add an `Intent:` trailer saying why the change is needed.
5. If commit hooks fail, fix only their findings, re-stage, and retry. Stop
   after three failed commit attempts and report the evidence.

Do not bypass hooks. Completion requires a created commit.
