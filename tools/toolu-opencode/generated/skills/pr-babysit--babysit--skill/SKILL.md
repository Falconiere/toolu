---
description: Use when the user explicitly asks Codex, or an authorized verified execution handoff invokes it, to monitor and autonomously clear the current branch's pull request until CI, review threads, and the review-bot verdict are all green.
name: pr-babysit--babysit--skill
---

# Babysit a PR

This no-argument invocation explicitly authorizes one durable babysitting goal
for the current repository and PR. A verified execution handoff is sufficient authorization
when execution already confirmed delivery authorization, GitHub
auth, a non-default branch, and this installed plugin; do not ask again or
introduce handoff arguments. Read [the canonical workflow](../../workflows/babysit.md)
completely and follow only its Codex controller branches plus every shared
strict-clearance step. Each tick is one command — the shipped
`scripts/babysit-tick.sh` under this plugin's root (`../../scripts/babysit-tick.sh`
relative to this file), with `--state-file` set to the Codex slot path; its output contract is [references/helper.md](references/helper.md).
Trust that result: never write a polling script or controller of your own,
never re-fetch with ad-hoc `gh` calls what the result already reports, and act
through `reply-thread.sh`, `resolve-thread.sh` and `record.sh`.

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
