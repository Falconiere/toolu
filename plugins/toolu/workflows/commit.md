# Commit all changes

Commit the current working tree without broad re-exploration.

1. Delegate this bounded workflow to the configured `quick-task` mechanical
   agent when that agent is available. Otherwise execute it in the current
   thread. Use the active host mapping in `workflows/host-mapping.md`.
2. Run only `git status` and `git diff --stat` to establish scope. Explore more
   only to fix a reported failure.
3. Stage all changes with `git add -A`; staged checks depend on index state.
4. Run the project's prescribed check, lint, and test command when one exists
   in repository instructions, package scripts, a Makefile, or toolchain
   configuration. If none exists, record that and continue.
5. On failure, fix only the reported issue, re-stage, and rerun every required
   gate until green.
6. Commit with a concise Conventional Commit subject of at most 72 characters.
   Group a body by package only when it adds useful context.
7. If commit hooks fail, fix only their findings, re-stage, and retry. Stop
   after three failed commit attempts and report the evidence.

Do not bypass hooks. Completion requires a created commit and green gates.
