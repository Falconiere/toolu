---
description: Commit all changes — run gates, stage, commit with conventional message.
agent: build
subtask: true
---

# Commit All Changes
Follow this exact sequence with NO unnecessary exploration:
0. Delegate to a subagent for commit workflow execution.
1. Run `git status` and `git diff --stat` only (no broad re-exploration unless fixing reported failures).
2. Stage all changes with `git add -A` (staged checks rely on index state).
3. Enforce hard gates on staged files before commit: run the project's own check/lint/test command if one exists (e.g. a `check`/`lint` script in `package.json`, a Makefile target, `cargo clippy`, or whatever the project's docs prescribe). If the project defines no check command, skip this step.
4. If any gate fails, fix only reported failures, re-stage, and re-run all gates until green.
5. Commit with concise conventional message (subject <=72 chars; body grouped by package when useful).
6. If hooks fail, fix only hook-reported issues, re-stage, and retry (max 3 retries).
7. Do not mark task complete unless all gates pass.
