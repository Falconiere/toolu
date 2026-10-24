---
name: epic-orchestrator
description: Drive a GitHub, Jira, or Linear epic to done with the resident `toolu epic` engine. Each open sub-issue gets a herdr worktree and a worker agent (Claude Code, Codex, Cursor Agent, or Muse Code) that delivers its PR; the engine merges and closes the epic. Runs from any of those four hosts. Use to work, resume, or stop an epic — "work epic #248", "dry-run the epic", "which issues can run in parallel", "where is the epic at". Not for a single issue.
---

# Epic orchestrator

You start an epic, then handle only the judgments the engine raises. The
resident engine (`toolu epic`) does everything deterministic: it launches each
batch, routes hosts, checks each PR every 3 minutes, rebases, merges, cleans
up, refreshes the graph and closes the epic. **The engine merges, not you.**
Never run a gate, merge, launch or teardown yourself in the normal flow, nor
write product code. Keep your context small: a run lasts hours.

## Inputs

`<epic>`: a GitHub issue URL, `owner/repo#N` or `#N`; a Jira key or URL
(`jira:KEY-12`, needs `JIRA_BASE_URL` plus `JIRA_PAT` or `JIRA_EMAIL` and
`JIRA_API_TOKEN`); a Linear issue or project (`linear:ENG-12`, needs
`LINEAR_API_KEY`). Jira and Linear items name their code repo with a
`repo:owner/name` label or a `Repo: owner/name` line, else `--repo`.

Options: `--max N` live workers (default 3); `--hosts claude:2,codex:2`
(hosts `claude`, `codex`, `cursor`, `muse`; default `claude`); `--safe`
keeps each host's approval prompts on; `--no-jev` routes by heuristics;
`--dry-run` plans and launches nothing ("show me the plan", "what can run in
parallel" and "don't launch yet" mean `--dry-run`). Subcommands: `status`,
`stop`.

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

**This session.** Run `toolu --host <host> doctor` for the host you run on
(`claude`, `codex`, `cursor`, `muse`) and stop on its first failure. Name the
host: toolu can't detect every host from its shell. Then stop and report the
first of these that fails:
- `HERDR_ENV` is `1` (herdr control works only from a herdr pane), and
  `herdr status` answers.
- `toolu epic status` answers (`"engine":"down"` is fine; `start` starts it).
  `Operation not permitted` from it or from `herdr status` means your host's
  sandbox blocks the engine and herdr sockets and the epic state directory,
  so every step below would fail the same way. Run your commands outside the
  sandbox. On Claude Code, disable the sandbox for those calls. Start Codex
  with `--sandbox danger-full-access`, or approve its escalation. Start
  Cursor Agent with `--sandbox disabled`. Start Muse Code with
  `--disable-sandbox`, or approve its one-time unsandboxed run.
- `gh auth status` succeeds, and `gh api rate_limit` shows core above 1000.
- The tracker variables above are exported (a Jira or Linear epic).
- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,
  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies.

**Each worker host in `--hosts`.** Run these checks in order. Stop at a
host's first failure and print its fix:
1. Its CLI is on `PATH`, and it is logged in:

   | Host | CLI | Logged in when | Fix |
   |---|---|---|---|
   | `claude` | `claude` | `claude auth status` shows `"loggedIn": true` | `claude auth login` |
   | `codex` | `codex` | `codex login status` prints `Logged in` | `codex login` |
   | `cursor` | `cursor-agent` | `cursor-agent status` prints `Logged in` | `cursor-agent login`; for `Your macOS login keychain is locked`, `security unlock-keychain` |
   | `muse` | `muse` | `META_API_KEY` is set, or its auth file exists: `MUSE_AUTH_PATH`, else `$XDG_CONFIG_HOME/muse/auth.json`, else `~/.config/muse/auth.json` | `muse login` |

2. `toolu plugins list --host <host>` succeeds and shows `toolu`,
   `toolu-review`, `pr-babysit`, `brainstorm` and `delivery-flow` installed:
   none of them on a `not installed` row. Fix:
   `toolu plugins install delivery-flow --host <host>`. If the command
   itself fails, your `toolu` is too old for that host: check the `binary`
   line of this session's doctor.
3. `toolu --host <host> doctor` prints no `fail` line and no `warn plugins`
   line. A `warn plugins` line means the plugins' hooks won't run, and it names
   the fix. For example, an unapproved Muse plugin prints
   `warn plugins: toolu capabilities need approval; muse plugins approve toolu`.
   Doctor exits 0 on a warning, so read the lines, not the exit code. One
   exception: a `fail skew` line that names only plugins outside those five
   (retired ones such as `context7`) is a note, not a stop.

Report one line per host, `ready` or `not ready: <fix>`. Launch nothing while
a host in `--hosts` is not ready. Drop that host only if the user agrees.

## 2. Plan

```bash
toolu --json epic graph <epic> --max 3 --dry-run > /tmp/epic-graph.json
toolu epic route --graph /tmp/epic-graph.json --hosts claude:2,codex:1 --dry-run
```

Add `--tracker jira --repo owner/name` for Jira or Linear, `--no-jev` to the
route when passed. Show both tables: ready issues and the launch batch, blocked
issues with their blockers, each issue's host, model and effort. Then:
- `complete: true`: nothing to launch; say so and stop.
- a non-empty `cycle`: report it and ask which edge is wrong.
- `external_blocked`: name the outside blockers; the engine never launches them.
- `missing_checkouts`: the engine clones those repos; tell the user.

**A dry run stops here.** It wrote nothing.

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
- **Codex:** one tool call cannot block for hours, so loop:
  run `toolu --json epic wait --epic <epic-key> --max-seconds 480` in the
  foreground, handle what it returns, and run it again while any issue is active.
- **Cursor Agent:** the same foreground loop:
  `toolu --json epic wait --epic <epic-key> --max-seconds 480`, again while
  any issue is active. Its shell runs a 480-second call to the end.
- **Muse Code:** the same foreground loop:
  `toolu --json epic wait --epic <epic-key> --max-seconds 480`, again while
  any issue is active. If the call returns a session id before it ends, don't
  poll that id: Muse delivers the result on its own and wakes you.

It returns one judgment of this epic or an engine-wide one (`kind`, `key`, `note`, `pending` for `blocked`) or
`{"state": "waiting"}` at the limit; only a `waiting` at once means no engine
runs (`toolu epic status`, then `toolu epic start "$STATE_DIR"`). A worker report
or a settled cause closes an item; ack the rest once handled.

| kind | action |
|---|---|
| `needs-human` | If the issue, epic or code answers the `note`, `toolu epic answer <key> "<answer>"`; else ask the user and relay it the same way. |
| `failed` | Read the note (`herdr agent read <key> --lines 80` if unclear). `toolu epic answer <key> "<new direction>"`, or escalate. Notes `uncertain launch`, `merge uncertain`, `merge refused` and, on the epic key, `epic close uncertain`: follow `references/recovery.md`. |
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

The engine closes the epic, posting its metrics beside the TypeScript baseline (a Linear project stays open: tell the user).
Report PRs merged, admin merges, hosts and models, what needed a human, and `toolu epic summary <epic>` (measures, budgets, token use).

## Status and stop

- **Status** ("where is the epic at"): print `toolu epic status <epic-key>`,
  `toolu epic queue <epic-key>` and `http://127.0.0.1:7717/` (peer engines: `toolu epic status --fleet`); never `toolu epic wait`.
- **Stop:** `toolu epic pause <epic-key>` stops launches, rebases and merges at
  once; workers finish their step and go idle. Then stop waiting.
  `toolu epic resume <epic-key>` continues. Tear an issue down only when the
  user asks (`references/recovery.md`).

## Guardrails

- **Progress is never lost.** Workers commit, push and report each phase. The
  engine snapshots every active worktree to `refs/epic-wip/<key>` every 15
  minutes and before any cleanup, journals each action before it acts, and
  never starts a second agent for an uncertain launch.
- **Budgets.** Launches and merges wait while GitHub core is under 1000
  (`EPIC_GH_CORE_FLOOR`); API calls retry with backoff and honor resets.
- **Lean.** Trust `toolu epic` output; don't re-fetch it with `gh`, read
  worker transcripts wholesale, or micro-manage workers. Failures in herdr,
  git, gh or the engine: `references/recovery.md`.
