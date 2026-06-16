---
name: review
description: Project-tuned pre-push code review that mirrors the CI review bot's checklist so the bot finds nothing on first push. Reviews the branch diff for correctness, security, performance, test coverage, doc accuracy, and tight assertions, then records a clean push-review state. Use before pushing a feature branch, when asked to "review before push", or when pr-babysit needs a reviewer. Explicit — does NOT auto-fire on edits.
---

# code-review:review

A pre-push reviewer tuned to what this repo's CI review bot (the `claude[bot]`
verdict comment) flags — run it locally so the bot's verdict is clean on the
first push instead of bouncing low/nit findings back as rework.

## What it reviews

Review `git diff <base>...HEAD` against these dimensions. Every finding blocks
(the gate requires zero) — fix in code, do not suppress:

1. **Correctness** — logic, edge cases, error handling (no swallowed errors, no
   `@ts-ignore`/`eslint-disable`/`#[allow]` papering over a real problem).
2. **Security** — input validation, injection, secrets, unsafe file/symlink ops.
3. **Performance** — hot paths (e.g. per-render/per-hook work), needless spawns.
4. **Test coverage for every NEW behavior** — a new code path without a colocated
   real-data test is a finding. (The bot flagged a missing bats for an orphan
   sweep on a prior PR — catch that class here.)
5. **Doc/comment accuracy** — comments must match behavior; e.g. no "one-time" on
   a block that runs every invocation; no stale paths after a move.
6. **Tight test assertions** — assert the full identity, not a loose suffix
   (`*/statusline/statusline.sh`, not `*/statusline.sh`).
7. **In-session migration WARNs** — a breaking change (moved path, removed symlink)
   must surface an actionable in-session hint, not a silent failure later.

## How to run

1. Resolve the diff: `git diff --no-color <base>...HEAD` (base = the push-review
   gate's base; the helper below resolves it the same way).
2. Review every changed hunk against the checklist. Read surrounding code and
   grep for usage before claiming a finding — no speculative nits.
3. Fix accepted findings in code, re-review until none remain.
4. Record the clean state for the push-review gate:

   ```bash
   bash "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/code-review/write-state.sh" \
     --findings-count 0 --reviewers '["code-review:review"]'
   ```

   `write-state.sh` is published as a symlink at
   `${CLAUDE_CONFIG_DIR:-$HOME/.claude}/code-review/write-state.sh` by the plugin's
   SessionStart hook (refreshed every session). `$CLAUDE_CONFIG_DIR` IS exported into
   the Bash tool's subshell; `$CLAUDE_PLUGIN_ROOT` is **NOT** — it is only set for
   hook subprocesses, so the plugin-root path expands to an empty string from a
   Bash tool call.

   It computes the gate's exact `diff_sha`/`base`/`slug`, bumps `review_round`,
   and writes `.claude/tmp/push-review/<branch-slug>.json` atomically. Harmless
   no-op when the toolu push-review gate is not installed (the file goes unread).

If findings remain that you cannot fix (e.g. needs a human decision), record them
with `--findings-count <n> --findings '<json>'` instead of 0 — the gate will then
keep blocking the push, which is correct: open findings are not done.
