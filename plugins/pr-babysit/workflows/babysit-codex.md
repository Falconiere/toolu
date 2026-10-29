# Codex controller

### Codex start or resume

Codex keeps no background command between turns. A user invocation of `$pr-babysit:babysit` is the
explicit request required to create a durable goal.

1. Call `get_goal`. The objective is exactly `Babysit <OWNER>/<REPO>#<NUMBER>
   until CI, review threads, and the review-bot verdict are clear`.
2. If no goal is active, call `create_goal` with that objective. If the matching
   goal is already active, resume it. If a different goal is active, stop and
   report the collision; never replace another objective implicitly. This
   enforces **one active goal per repository/PR** and one babysit target per
   thread.
3. Set `STATE_FILE="$REPO_ROOT/.codex/tmp/pr-babysit/$SLOT.json"`. The helper
   creates its parent and initializes the state on the first tick; an
   existing active file for the same slot is resume state. Never glob or inspect
   sibling slots.
4. Run one complete clearance cycle (Steps 1–6). Then run
   `toolu babysit wait --repo "$OWNER/$REPO" --pr "$NUMBER" --state-file "$STATE_FILE"`,
   which returns within 55 s: a tick result that needs you → another cycle;
   the waiting document → yield with the goal active. A later goal
   continuation runs `wait` again. Never busy-poll or sleep yourself.
5. Call `update_goal(status="complete")` only at the Success stop. Use
   `update_goal(status="blocked")` only at a genuine Escalation stop after the
   same human-only blocker has recurred for the host-required three consecutive
   goal turns. Pending CI is not blocked and never completes the goal.

### Codex cancel

On `stop` or `cancel`, resolve only the current branch's slot. Validate that the
state path is exactly below `$REPO_ROOT/.codex/tmp/pr-babysit/` and that any
worktree recorded in it belongs to this exact slot. Remove that worktree with
native `git worktree remove <exact-path>` only when clean, and run
`toolu babysit stop --state-file "$STATE_FILE"`, which cleans up a herdr
worktree and marks the state `cancelled`; a failure stops cleanup and is
reported. Then
tell the user to cancel the active goal with Codex's goal control (goal
cancellation is user/system controlled, not an `update_goal` status). Never
mark cancellation complete.

---
