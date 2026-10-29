---
name: babysit
description: Use when the user explicitly asks Codex, or an authorized verified execution handoff invokes it, to monitor and autonomously clear the current branch's pull request until CI, review threads, and the review-bot verdict are all green.
---

# Babysit a PR

This no-argument invocation explicitly authorizes one durable babysitting goal
for the current repository and PR. A verified execution handoff is sufficient authorization
when execution already confirmed delivery authorization, GitHub
auth, a non-default branch, and this installed plugin; do not ask again or
introduce handoff arguments. Read [the canonical workflow](../../workflows/babysit.md)
and its linked Codex controller completely; load its fix procedure whenever
directed. Follow every shared strict-clearance step. Each check is one command — `toolu babysit wait` (or
`toolu babysit tick` for a check right now) with `--state-file` set to the Codex
slot path; its output contract is [references/helper.md](references/helper.md).
Trust that result: never write a polling script or controller of your own,
never re-fetch with ad-hoc `gh` calls what the result already reports, and act
through `toolu babysit reply`, `toolu babysit resolve` and `toolu babysit record`. Fix items go
through `toolu babysit route-fix --host codex` and, when it dispatches to herdr,
`toolu babysit dispatch-fix start` then `toolu babysit dispatch-fix wait --timeout-seconds 45` — one
bounded wait per continuation. A fixer launch that is due runs first in the
call; it normally takes seconds and is bounded when an agent fails to start.

Use `get_goal` before `create_goal`; keep one active goal for the resolved
repository/PR. Continue with bounded cycles: `toolu babysit wait` returns
within 55 seconds, the slot state persists, and later goal continuations
resume by running it again. Pending checks are not completion or blockage.

Call `update_goal` with `complete` only after the same-cycle success audit proves
CI green, zero unresolved threads, and an approved zero-finding bot verdict.
Call it with `blocked` only for a genuine human-only escalation after the same
blocker has met Codex's consecutive-goal-turn threshold. For `stop` or `cancel`,
perform the canonical local cleanup and tell the user to cancel through Codex's
goal control; cancellation is not a completion status.
