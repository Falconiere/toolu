# Recovery playbook

Start every repair from what the engine recorded, not from ad-hoc `gh` calls:
`toolu epic status <key>` (one worker: stage, phase, PR, agent state, pending
action, attention), the journal (`/api/v1/events` on the dashboard,
`http://127.0.0.1:7717/`, every action with its intent and outcome), and
`toolu doctor` (binary, config, plugins and tools). Then fix the cause and let
the engine retry, or run the one repair below. A timeout during an agent start
or prompt is uncertain: it is never retried as a fresh session.

## Engine

- **No engine running** (`toolu epic wait` answers `waiting` at once): run
  `toolu epic start <state_dir>` again with the run's flags. The engine reloads
  its journal, finishes what was in flight and relaunches nothing whose start
  is uncertain.
- **Context compacted or session restarted:** re-invoke the skill on the same
  epic; `toolu epic status` shows where every issue is.
- **Two orchestrators on one epic:** never. Before starting, check
  `herdr agent list` for live agents named `<repo>-<n>` that `toolu epic
  status` does not know. If you find any, stop and ask.
- **Paused by mistake:** `toolu epic resume <epic-key>`.

## herdr

- **`HERDR_ENV` unset:** the orchestrator is not inside herdr. Stop and tell
  the user to start the session from a herdr pane.
- **`herdr-error` that persists:** `herdr status`; the engine backs off 30
  seconds, doubling to 5 minutes, and recovers once herdr answers.
- **`worktree_create_failed ... already exists`:** launch reopens worktrees
  git knows. For a stale directory git does not list, `git -C <checkout>
  worktree prune`, then let the engine launch again. Delete the directory only
  if `git -C <dir> status` shows no work worth keeping.
- **An untrusted-repository error:** ask the user to confirm the repo; only
  then use herdr's `--trust-repository` (never as a routine retry).
- **`agent_not_ready` or a start timeout:** read the pane (`herdr pane read
  <pane> --source recent-unwrapped --lines 60`). Approve only the standard
  workspace-trust prompt for this worktree; otherwise ask the user.
- **`agent_prompt_stalled`:** this does not prove the prompt was lost. `herdr
  agent read` it before sending anything again.
- **Name collision** (`<key>` is live in another pane): `herdr agent get
  <key>`. Reuse it only if host, pane, worktree and captured session match.

## Relaunch

`failed` with an `uncertain launch` note, a `reopened` issue, or a worker that
must move host. Inspect `attempt_id`, `pane_id`, `session_id`, `prompt_state`
and `lifecycle_error` in `toolu epic status <key>` first, then:

```bash
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --reprompt   # a verified session: prompt it again
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --replace    # shut it down verifiably, start fresh
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --force      # a reopened issue the graph blocks
```

Never delete a lease to make capacity look free. A wrong model name (the agent
exits right after start): check `codex debug models`, `cursor-agent
--list-models` or `opencode models`, fix `routing.json`, then `--replace`.

## Merge and teardown

- **`failed`, note `merge uncertain`:** never resend a merge. Check the PR;
  merged or not, `toolu epic ack <key>` and let the engine's next PR check
  settle it.
- **`merge-ready`** (a deferring merge script, a test seam), or a repair by
  hand: run the gate below, the one the engine runs.
- **`cleanup-incomplete`:** the teardown kept the workspace because a job was
  still running or a snapshot or removal failed (`cleanup_error`). Fix that,
  then `toolu epic finish <state_dir> <key>` and `toolu epic ack <key>`.
- **Tear an issue down without a merge** (only when the user asks):
  `toolu epic finish <state_dir> <key> --abandon` keeps the branch.
- **Close the epic by hand:** `toolu epic close --graph <state_dir>/graph.json`
  (`--dry-run` prints the summary only). Never close it twice.

```bash
toolu epic gate --pr <repo>#<pr> --issue <ref> --epic <epic> --state-dir <state_dir> --key <key>
toolu epic gate --pr <repo>#<pr> --issue <ref> --epic <epic> --state-dir <state_dir> --key <key> --merge
```

The gate checks that the PR is open, contains the base tip, has every check
green and zero unresolved threads. Re-run it with `--merge` only on a `merge`
verdict: it pins the verified head and uses `--admin` only on a
protection-only refusal. `--admin` refused means admins are enforced or you
lack the right: tell the user.

## GitHub, trackers and hosts

- **Clone fails:** `gh auth status` and access to that org. Report it for that
  issue only; the others keep moving.
- **A check failing for reasons outside the PR:** the engine's babysit
  re-runs failed jobs. The same check failing 3 times on unrelated code:
  escalate with the run URL.
- **A sub-issue closed without a PR** ("not planned", duplicate): counts as
  done; mention it in the final summary.
- **`host-limited`:** the engine cools the host down and moves the worker to a
  free host; nothing to do unless every host is out.
- **Rate limited past the 900 s cap:** wait until the printed time; the
  engine retries its refresh every 3 minutes.
- **Jira `no-done-transition`:** no transition into Done; tell the user to move
  it by hand.

## Lost work

Every snapshot is in `git -C <checkout> reflog refs/epic-wip/<key>`. Recover
on a new branch (`git -C <checkout> checkout -b recover/<key>
refs/epic-wip/<key>`), compare with the issue branch, and have the worker
continue from it; push with `--force-with-lease` only after checking the remote
has nothing newer.

## Workers

- **Keeps failing in the same phase:** one concrete redirect through
  `toolu epic answer <key> "<direction>"`. If it fails again, tear it down
  with `--abandon`, ask the user, and hold every issue that depends on it.
- **Review loops without converging:** after 3 rounds, have the worker record
  the disagreement as a decision in its spec and move on.
- **Off scope** (its diff touches other issues' areas): send it back to its
  plan with the files named; scope creep causes conflicts for other workers.
