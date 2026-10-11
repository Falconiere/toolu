---
description: "Use when the user explicitly asks, or an authorized verified execution handoff invokes it, to monitor and autonomously clear the current branch's pull request until CI, review threads, and the review-bot verdict are all green."
name: "pr-babysit-babysit-73c340c6"
---

# Babysit a PR

This no-argument invocation explicitly authorizes babysitting the current
repository's pull request. A verified execution handoff is sufficient authorization
when execution already confirmed delivery authorization, GitHub
auth, a non-default branch, and this installed plugin; do not ask again or
introduce handoff arguments. Read [the canonical workflow](../../resources/pr-babysit/workflows/babysit.md)
completely and follow its OpenCode controller plus every shared strict-clearance
step.

Each check is one command, `toolu babysit wait` (or `toolu babysit tick` for a
check right now) with
`--state-file "$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json"`; its output
contract is [references/helper.md](references/helper.md). Trust that result:
never write a polling script or controller of your own, never re-fetch with
ad-hoc `gh` calls what the result already reports, and act through
`toolu babysit reply`, `toolu babysit resolve` and `toolu babysit record`.
Fix items go through `toolu babysit route-fix --host opencode` and, when it
dispatches fixer agents, `toolu babysit dispatch-fix start` then
`toolu babysit dispatch-fix wait --timeout-seconds 45` — one bounded wait per cycle.

OpenCode has no cron, goal or background command, so this turn is the
controller. After each cycle, run `toolu babysit wait` again (it returns within
55 s), until the Success or Escalation stop. Pending checks are neither
completion nor blockage. If the turn ends first, invoking this again resumes
from the state file.

Stop with success only after the same-cycle success audit proves CI green, zero
unresolved threads, and an approved zero-finding bot verdict; escalate only for a
genuine human-only blocker. For `stop` or `cancel`, run the workflow's OpenCode
cancel; cancellation is not completion.
