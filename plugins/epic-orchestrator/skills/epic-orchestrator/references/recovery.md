# Recovery playbook

Read the section that matches the failure. For every case: fix the cause, then
re-run the same script. All the scripts are idempotent.

## herdr

- **`HERDR_ENV` unset**: the orchestrator isn't running inside herdr. Stop and
  tell the user to start the session from a herdr pane.
- **`worktree_create_failed ... already exists`**: a worktree directory for
  that branch is left over. `launch-issue.ts` reopens worktrees that git knows
  about. If git doesn't list it (a stale directory), run
  `git -C <checkout> worktree prune` and launch again. Don't delete the
  directory unless `git -C <dir> status` shows no work worth keeping.
- **An untrusted-repository error**: ask the user to confirm the repo. Only
  then may you pass `--trust-repository` (herdr's rule: never as a routine retry).
- **`agent start` times out or returns `agent_not_ready`**: read the pane
  (`herdr pane read <pane> --source recent-unwrapped --lines 60`). A first-run
  trust or permission dialog in a new worktree is common. Approve it only if
  it's the standard Claude workspace-trust prompt for this worktree. Otherwise
  ask the user.
- **`agent_prompt_stalled`**: this doesn't prove the prompt was lost.
  `herdr agent read` it before re-sending. Never blindly submit the brief
  prompt twice.
- **Name collision** (`<key>` is already a live agent in some other pane):
  inspect it with `herdr agent get <key>`. If it's a previous worker for the
  same issue, reuse it. Otherwise ask the user.

## git / GitHub

- **Clone fails**: check `gh auth status` and whether this account has access
  to that org. Report it for that issue only and keep the other issues moving.
- **Merge refused, not by protection** (the gate returns `merged: false` with
  an error): most often the head moved between gate and merge because the
  worker pushed again. Re-run the gate without `--merge` and follow the new
  verdict.
- **`--admin` refused**: admins are enforced, or you lack admin rights. Tell
  the user. The PR is gate-green and needs their approval or merge.
- **A failing check that isn't the PR's fault** (flaky or infra): let the
  worker's babysit handle it. It re-runs failed jobs as part of its CI step.
  If the same check fails 3 times on unrelated code, escalate with the run URL.
- **A sub-issue closed without a merged PR** ("not planned" or a duplicate):
  it counts as done for the graph. Mention it in the final epic summary.
- **A sub-issue reopened after its merge**: the graph shows it `ready` again.
  Read the reopening comment before relaunching. A regression usually deserves
  a fresh branch (`--force` if the graph blocks it) and a new PR.

## Workers

- **The worker reports `ready` but the gate says `fix` with unresolved threads**:
  new review comments arrived after babysit's last tick. Send `FIX:`. The
  worker re-runs babysit.
- **The worker keeps reporting `failed` in the same phase**: give one concrete
  redirect based on its note (a different approach, a smaller slice, or an
  explicit decision on an ambiguity). If it fails again, stop that issue with
  `--abandon` and ask the user. Also hold every issue that transitively
  depends on it.
- **Spec or plan review loops without converging**: after 3 review rounds,
  have the worker record the remaining disagreement as an explicit decision in
  the spec and move on. Unbounded review is worse than a documented call.
- **The worker went off-scope** (its diff touches areas outside its issue):
  send it back to the plan with the specific files named. Scope creep causes
  rebase conflicts for the other workers.

## Orchestrator

- **Context compacted or session restarted**: re-invoke the skill on the same
  epic. It resumes from the state dir: the graph shows `in_flight` issues, and
  the launcher reattaches or relaunches them.
- **Two orchestrators on one epic**: never. Before launching, check
  `herdr agent list` for live agents named `<repo>-<n>` from this epic that
  your state dir doesn't know about. If you find any, stop and ask.
