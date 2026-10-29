---
name: epic-orchestrator
description: Run, resume, inspect or pause a GitHub, Jira or Linear epic through the resident toolu engine. Use dry-run for planning questions; not for a single issue. Triggers include "dry-run the epic" and "where is the epic at".
---

# Epic orchestrator

The resident `toolu epic` engine owns launches, PR checks, rebases, cleanup
and closure. **The engine merges, not you.** Handle its judgments; do not
write product code. Read [recovery](references/recovery.md) only for a matching failure.

Route status and stop requests directly to their section below; they do not
authorize a new run. Planning questions use dry-run and stop after the plan.

## Inputs

`<epic>`: a GitHub issue URL, `owner/repo#N` or `#N`; a Jira key or URL
(`jira:KEY-12`, needs `JIRA_BASE_URL` plus `JIRA_PAT` or `JIRA_EMAIL` and
`JIRA_API_TOKEN`); a Linear issue or project (`linear:ENG-12`, needs
`LINEAR_API_KEY`). Jira and Linear items name their code repo with a
`repo:owner/name` label or a `Repo: owner/name` line, else `--repo`.

Options: `--max N` live workers (default 3); `--hosts claude:2,codex:2`
(hosts `claude`, `codex`, `cursor`, `muse`); `--safe` keeps each host's
approval prompts on; `--no-jev` routes by heuristics; `--dry-run` plans and
launches nothing ("show me the plan", "what can run in parallel" and "don't
launch yet" mean `--dry-run`). Subcommands: `status`, `stop`.

## Authorization boundary

Invoking this skill on an epic authorizes, for that epic's sub-issues only
(unattended workers included: the brief and toolu's hooks hold the same
limits the approval prompts would):
- create worktrees and agents
- push issue branches, using `--force-with-lease` after a rebase
- open PRs and run babysit
- merge PRs that pass the merge gate
- close delivered sub-issues (GitHub close, Jira transition to Done, Linear
  completed state), then tick and close the epic

`--admin` is allowed only when branch protection (for example, a required
approval on a solo repo) is the only thing blocking a gate-green PR. `--admin`
never overrides a failing check, a pending check, or an unresolved thread.

Not authorized: pushing to `main`, or touching PRs, branches or worktrees
outside the epic. If a worker's permission prompt asks for anything outside
the authorized list, ask the user before approving it.

## 1. Preflight

Use [`references/preflight.md`](references/preflight.md) for host-specific fixes.
Run `toolu doctor` with `--host <host>` first; stop with the fix for the first failure:
- `HERDR_ENV` is `1`, and `herdr status` and `toolu epic status` answer.
  `Operation not permitted` from either means your host's sandbox blocks the engine: run outside it.
- `gh auth status` succeeds and `gh api rate_limit` shows core above 1000; a Jira or Linear epic has its tracker variables.
- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,
  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies.
- Each host in `--hosts` is logged in, `toolu plugins list --host <host>` shows those five
  plugins, and `toolu --host <host> doctor` prints no `fail` or `warn plugins` line (an
  unapproved Muse plugin prints `muse plugins approve toolu`): `ready` or `not ready: <fix>`.

## 2. Plan

```bash
toolu --json epic graph <epic> --max 3 --dry-run > /tmp/epic-graph.json
toolu epic route --graph /tmp/epic-graph.json --hosts claude:2,codex:1 --dry-run
```

For Jira or Linear, use the matching `--tracker jira` or `--tracker linear`
and `--repo owner/name` when needed. Add `--no-jev` to route when passed. Show both tables: ready issues and the launch batch, blocked
issues with their blockers, each issue's host, model and effort. Then:
- `complete: true`: nothing to launch; say so and stop.
- a non-empty `cycle`: report it and ask which edge is wrong.
- `external_blocked`: name the outside blockers; the engine never launches them.
- `missing_checkouts`: the engine clones those repos; tell the user.

**A dry run stops here:** no engine state, workers or tracker mutations;
the scratch graph file is local planning output.

## 3. Run

```bash
STATE_DIR=$(toolu --json epic graph <epic> --max 3 --save | jq -r .state_dir)
toolu epic route --graph "$STATE_DIR/graph.json" --hosts claude:2,codex:1
toolu epic start "$STATE_DIR"    # add --safe and --no-jev when the user passed them
```

The basename of `$STATE_DIR` is the `<epic-key>`. `start` registers the
epic, keeps `--safe` and `--no-jev` until a start without them, and starts the
engine. Tell the user the dashboard, `http://127.0.0.1:7717/`, and which
workers run where (`toolu epic status <epic-key>`; each has a herdr workspace
named after its key). Resume: the same commands.

## 4. Wait for judgments

- **Claude Code:** run `toolu --json epic wait --epic <epic-key>` as a background Bash call
  (`run_in_background: true`) and end your turn. Its completion (a judgment,
  or `waiting` after 45 minutes) wakes you: handle it, then start the next one
  while any issue is active. Keep exactly one; answer the user meanwhile.
- **Codex:** run `toolu --json epic wait --epic <epic-key> --max-seconds 480`
  through yielding execution (tool returns within 60 seconds), handle its result,
  and run it again while any issue is active. Without yielding, use a shorter wait.
- **Cursor Agent** and **Muse Code:** the same bounded loop. On Muse, a call
  that returns a session id delivers its result on its own and wakes you: don't poll it.

It returns one judgment of this epic or an engine-wide one (`kind`, `key`, `note`, `pending` for `blocked`) or
`{"state": "waiting"}` at the limit; only a `waiting` at once means no engine
runs (`toolu epic status`, then `toolu epic start "$STATE_DIR"`). A worker report
or a settled cause closes an item; ack the rest once handled. To redirect a live worker with no item open, use `toolu epic answer <key> "<direction>"`; it is journaled and shown in `toolu epic status`.

| kind | action |
|---|---|
| `needs-human` | If the issue, epic or code answers the `note`, `toolu epic answer <key> "<answer>"`; else ask the user and relay it the same way. |
| `failed` | Read the note (`herdr agent read <key> --lines 80` if unclear). `toolu epic answer <key> "<new direction>"`, or escalate. Notes `uncertain launch`, `merge uncertain`, `merge refused`, `rebase push uncertain` and tracker close failures on the epic key: follow `references/recovery.md`. |
| `blocked` | Read `pending`. Inside the boundary: approve with `herdr agent send-keys`; else ask the user. Then `toolu epic ack <key>`. |
| `stall` | The engine sent `STATUS?`. Read the reply, then `toolu epic answer <key> "<nudge>"` or `toolu epic ack <key>` to wait one more stall length. |
| `babysit` | The PR check needs the worker or a human (the note says why). `toolu epic answer <key> "<direction>"` or escalate, then `toolu epic ack <key>`. |
| `rebase` | The engine's `REBASE` prompt was not delivered: `toolu epic answer <key> "REBASE"`. |
| `coupling-hold` | When the overlapping issue in the note merges, or you judge the overlap false, `toolu epic release <key>`. |
| `relaunch-limit` | The agent vanished a third time. Escalate, then `toolu epic ack <key>`. |
| `reopened` | Read the reopening comment; relaunch per `references/recovery.md`, then `toolu epic ack <key>`. |
| `pr-closed` | Closed without merge: ask the user, then `toolu epic ack <key>`. |
| `cleanup-incomplete` | Fix the cause per `references/recovery.md`, then `toolu epic ack <key>`. |
| `refresh-failed` | The tracker failed three times (often credentials). Fix it; the engine retries and closes the item (check with `toolu epic status <key>`). |
| `herdr-error` | Run `herdr status`; the engine backs off and recovers. `toolu epic ack <key>`; escalate if it persists. |
| `dependency-cycle` | Ask which edge is wrong, fix it in the tracker, then `toolu epic ack dependency-cycle`. |
| `merge-ready` | Only a deferring merge script raises it (a test seam): see `references/recovery.md`. |
| `epic-complete` | `toolu epic ack <key>`, then report (step 5). |

## 5. Done

The engine closes the epic and posts metrics (a Linear project stays open:
tell the user).
Report PRs merged, admin merges, hosts and models, what needed a human, and `toolu epic summary <epic>` (measures, budgets, token use).

## Status and stop

- **Status** ("where is the epic at"): print `toolu epic status <epic-key>`,
  `toolu epic queue <epic-key>` and `http://127.0.0.1:7717/` (peer engines: `toolu epic status --fleet`); never `toolu epic wait`.
- **Stop:** `toolu epic pause <epic-key>` stops launches, rebases and merges at
  once; workers finish their step and go idle. Then stop waiting.
  `toolu epic resume <epic-key>` continues. Tear an issue down only when the
  user asks (`references/recovery.md`).

## Guardrails

- Preserve journals and `refs/epic-wip/<key>` snapshots; never duplicate an uncertain launch.
- **Budgets.** Launches and merges wait while GitHub core is under 1000
  (`EPIC_GH_CORE_FLOOR`); API calls retry with backoff and honor resets.
- **Lean.** Trust `toolu epic` output; don't re-fetch it with `gh`, read
  worker transcripts wholesale, or micro-manage workers. Failures in herdr,
  git, gh or the engine: `references/recovery.md`.
