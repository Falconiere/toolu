---
name: babysit
description: Use when the user explicitly asks Codex to monitor and autonomously clear the current branch's pull request until CI, review threads, and the review-bot verdict are all green.
---

# Babysit a PR

This invocation explicitly authorizes one durable babysitting goal for the
current repository and PR. Read [the canonical workflow](../../workflows/babysit.md)
completely and follow only its Codex controller branches plus every shared
strict-clearance step.

Use `get_goal` before `create_goal`; keep one active goal for the resolved
repository/PR. Continue with bounded cycles: use the native `wait` mechanism for
at most 60 seconds, persist the exact slot state, and allow later goal
continuations to resume. Pending checks are not completion or blockage.

Call `update_goal` with `complete` only after the same-cycle success audit proves
CI green, zero unresolved threads, and an approved zero-finding bot verdict.
Call it with `blocked` only for a genuine human-only escalation after the same
blocker has met Codex's consecutive-goal-turn threshold. For `stop` or `cancel`,
perform the canonical local cleanup and tell the user to cancel through Codex's
goal control; cancellation is not a completion status.
