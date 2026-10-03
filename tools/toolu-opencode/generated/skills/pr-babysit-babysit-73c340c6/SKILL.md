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

In every bash call, `PLUGIN_ROOT="$TOOLU_PLUGIN_ROOT_PR_BABYSIT"` and each helper
runs as `"$TOOLU_BUN" --no-env-file "$PLUGIN_ROOT/hooks/dist/<helper>.js"`, so a project `.env` never
reaches `gh`. Each tick is one command, `babysit-tick.js` with
`--state-file "$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json"`; its output
contract is [references/helper.md](references/helper.md). Trust that result:
never write a polling script or controller of your own, never re-fetch with
ad-hoc `gh` calls what the result already reports, and act through
`babysit-reply-thread.js`, `babysit-resolve-thread.js` and `babysit-record.js`.
Fix items go through `babysit-route-fix.js --host opencode` and, when it
dispatches fixer agents, `babysit-dispatch-fix.js start` then
`babysit-dispatch-fix.js wait --timeout-seconds 45` — one bounded wait per tick.

OpenCode has no cron or goal, so this turn is the controller. After a cycle that
ends `keep_going`, run `sleep <backoff.waitSeconds>` (never more than 60) in one
bash call and tick again, until the Success or Escalation stop. Pending checks are
neither completion nor blockage. If the turn ends first, invoking this again
resumes from the state file.

Stop with success only after the same-cycle success audit proves CI green, zero
unresolved threads, and an approved zero-finding bot verdict; escalate only for a
genuine human-only blocker. For `stop` or `cancel`, run the workflow's OpenCode
cancel; cancellation is not completion.
