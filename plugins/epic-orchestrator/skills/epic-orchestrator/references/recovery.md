# Recovery playbook

Start every repair from what the engine recorded, not from ad-hoc `gh` calls:
`toolu epic status <key>` (one worker: stage, phase, host, model, effort, PR,
last gate verdict, queue position, launches, phase history and recent events;
`toolu epic status` alone lists the open attention items), the journal
(`/api/v1/events` on the dashboard, `http://127.0.0.1:7717/`, every action with
its intent and outcome), and `toolu doctor` (binary, config, plugins and
tools). The launch record `<state_dir>/issues/<key>.json` holds the agent's
attempt, pane and session. Then fix the cause and let
the engine retry, or run the one repair below. A timeout during an agent start
or prompt is uncertain: it is never retried as a fresh session. An
`agent_pane_busy` refusal (the new pane's shell is not at its prompt yet)
starts nothing: the launch retries it for up to 30 s, then returns its lease
and waits for the engine to launch it again, with no `--replace`.

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
- **`needs-human`, note `codex is waiting on its hook review in pane <pane>`:**
  the worker sits on Codex's hook review, which the launch read from its pane.
  Review and trust the hooks in that pane (`t` trusts all), then
  `toolu epic launch … --replace`. If every Codex launch stops there, run
  `toolu plugins update --host codex`: a marketplace that follows `main` asks
  for the review again after each merge.
- **Name collision** (`<key>` is live in another pane): `herdr agent get
  <key>`. Reuse it only if host, pane, worktree and captured session match.

## Relaunch

`failed` with an `uncertain launch` note, a `reopened` issue, or a worker that
must move host. Inspect `attempt_id`, `pane_id`, `session_id`, `prompt_state`
and `lifecycle_error` in the launch record `<state_dir>/issues/<key>.json`
first, then:

```bash
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --reprompt   # a verified session: prompt it again
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --replace    # shut it down verifiably, start fresh
toolu epic launch --graph <state_dir>/graph.json --issue <ref> --force      # a reopened issue the graph blocks
```

Sometimes a prompt stalls before the host records its session, and the worker
runs once someone submits the prompt in its pane. The record then shows
`session_id: null` and a `prompt_state` other than `acknowledged`. When that
worker reports, the engine records it as `running` and sets its agent lease from
`uncertain` to `running`, so `toolu epic job` runs its gate. To repair the
record, run `toolu epic launch --graph <state_dir>/graph.json --issue <ref>`
with no recovery flag. It keeps the live worker and records the one session
the host wrote for the worktree since the attempt began. It does not prompt the
worker again, and it refuses if it finds no session or several. `--reprompt`
repairs the record the same way but also sends the worker its prompt again.

Never delete a lease to make capacity look free. A wrong model name (the agent
exits right after start): check `codex debug models` or `cursor-agent
--list-models`, fix `routing.json`, then `--replace`.

## Merge and teardown

- **`failed`, note `merge uncertain`:** never resend a merge. Check the PR;
  merged or not, `toolu epic ack <key>` and let the engine's next PR check
  settle it.
- **`failed`, note `merge refused`:** GitHub refused the pinned merge (often a
  push after the gate, or protection), and the engine holds the issue without
  resending. Fix the cause or ask the user, merge with the gate below once it
  says `merge`, then `toolu epic ack <key>`; the PR check sees the merge.
- **`failed` on the epic key, note `epic close uncertain`:** the epic may be
  closed already. Check it in the tracker and never close it twice; then
  `toolu epic ack <epic-key>`.
- **`merge-ready`** (a deferring merge script, a test seam), or a repair by
  hand: run the gate below, the one the engine runs.
- **`cleanup-incomplete`:** the teardown kept the workspace because a job was
  still running or a snapshot or removal failed (`cleanup_error`). A note
  starting `after <n> attempts: still exiting:` means the engine already
  retried: it names the agent that did not leave herdr or the worktree pids
  that stayed alive. Stop those first. Fix the cause, then
  `toolu epic finish <state_dir> <key>` and `toolu epic ack <key>`.
- **Tear an issue down without a merge** (only when the user asks):
  `toolu epic finish <state_dir> <key> --abandon` keeps the branch. It tells
  a running engine before and after the teardown, so the engine never
  relaunches the issue, and the abandoned issue frees its slot. If the engine
  refuses, nothing is torn down: wait for a merge or cleanup in progress, or
  replace an older engine with `toolu epic engine --replace`. An issue removed
  from the epic's tracker leaves the engine at its next refresh.
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
on a new branch (`git -C <checkout> checkout -b fix/recover-<key>
refs/epic-wip/<key>`, with `<key>` in lowercase so the name passes git-better),
compare with the issue branch, and have the worker
continue from it; push with `--force-with-lease` only after checking the remote
has nothing newer.

## Workers

- **Re-scoped while it idles** (no attention item): `toolu epic answer <key>
  "<new direction>"` delivers it, journals it and shows it in `toolu epic
  status`. It refuses only when the issue has no live agent. Never prompt the
  agent directly through herdr: the engine would not see the message.
- **Keeps failing in the same phase:** one concrete redirect through
  `toolu epic answer <key> "<direction>"`. If it fails again, tear it down
  with `--abandon`, ask the user, and hold every issue that depends on it.
- **Review loops without converging:** after 3 rounds, have the worker record
  the disagreement as a decision in its spec and move on.
- **Off scope** (its diff touches other issues' areas): send it back to its
  plan with the files named; scope creep causes conflicts for other workers.
