---
description: "Drive a GitHub, Jira, or Linear epic to done. Each open sub-issue gets its own herdr git worktree and agent (Claude Code, Codex, Cursor Agent, or OpenCode, routed by complexity), which runs delivery-flow from brainstorm through PR and pr-babysit. Independent issues run in parallel based on the dependency graph. Every branch is rebased on main, green PRs are merged automatically (--admin only when branch protection is the only thing blocking), and the epic closes when every sub-issue is closed. Use this whenever the user points at an epic, tracking issue, parent issue with sub-issues, or a checklist of linked issues and wants it worked, implemented, shipped, delivered, driven, resumed, or orchestrated. Examples — \"work epic #248\", \"ship all the sub-issues of <url>\", \"pick up the replication epic\", \"which issues in this epic can run in parallel\", \"dry-run the epic\", \"where is the epic at\". Not for a single standalone issue or PR."
name: "epic-orchestrator-epic-orchestrator"
---

# Epic orchestrator

You are the orchestrator. Your job is to turn one epic (GitHub, Jira, or
Linear) into merged GitHub PRs. Every open sub-issue gets its own herdr
worktree with an agent (the *worker*) inside it. Workers can run on Claude
Code, Codex, Cursor Agent, or OpenCode, several hosts at once. Workers write the code and babysit their PRs. You decide
the order, gate each merge, merge, and clean up. Don't write product code
yourself. An epic run lasts hours, so keep your context small: the scripts
handle the deterministic work, and a background watcher wakes you only when a
decision is needed.

```bash
# Claude / Cursor Agent: CLAUDE_PLUGIN_ROOT. Codex: PLUGIN_ROOT.
# OpenCode generated surface rewrites CLAUDE_PLUGIN_ROOT → TOOLU_PLUGIN_ROOT.
ROOT="${TOOLU_PLUGIN_ROOT}"
ROOT="${ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT}}}"
S="${ROOT}/scripts"
```

## Inputs

`<epic>` can be:
- GitHub: an issue URL, `owner/repo#N`, or `#N` (current repo).
- Jira: `https://<site>.atlassian.net/browse/KEY-12`, `jira:KEY-12`, or a bare
  `KEY-12` when only Jira is configured. Uses the toolu `jira` plugin's
  `jira.sh` for auth. Children are `parent = KEY-12` (or `"Epic Link"` on
  Server/DC); blockers are "is blocked by" / "depends on" links.
- Linear: an issue or project URL, `linear:ENG-12`, or a bare `ENG-12` when
  only `LINEAR_API_KEY` is set. Children are sub-issues (or project issues);
  blockers are `blocks` relations.

Jira and Linear hold the plan; the code still lives in GitHub repos. Each item
goes to the repo named by a `repo:owner/name` label or a `Repo: owner/name`
line in its description, else `--repo owner/name` (default: current repo).

Options:
- `--max N`: live issue agents at once (default 3).
- `--hosts claude:2,codex:2`: worker hosts and per-host caps (default
  `claude`, cap = `--max`). Hosts: `claude`, `codex`, `cursor` (cursor-agent),
  `opencode`.
- `--no-jev`: skip Jev complexity scoring (heuristic tiers instead).
- `--safe`: keep each host's approval prompts on. Default is unattended:
  `--dangerously-skip-permissions` (Claude), `--dangerously-bypass-approvals-and-sandbox`
  (Codex), `--yolo --trust --approve-mcps` (Cursor), `--auto` (OpenCode).
- `--kind`, `--model`, `--effort` override the route for one launch.
- `--dry-run` prints the plan without launching anything.
- Subcommands: `status` and `stop`.
Treat "show me the plan", "what can run in parallel" and "don't launch yet"
as `--dry-run`.

## Authorization boundary

Invoking this skill on an epic authorizes the following, for that epic's
sub-issues only (unattended workers included: the brief and toolu's hooks
hold the same limits the approval prompts would):
- create worktrees and agents
- push issue branches, using `--force-with-lease` after a rebase
- open PRs and run babysit
- merge PRs that pass the merge gate
- close delivered sub-issues (GitHub close, Jira transition to Done, Linear
  completed state), then tick and close the epic

`--admin` is allowed only when branch protection (for example, a required
approval on a solo repo) is the only thing blocking a gate-green PR.
`merge-gate.ts` enforces that rule. `--admin` never overrides a failing check,
a pending check, or an unresolved thread.

Not authorized: pushing to `main`, or touching PRs, branches or worktrees
outside the epic. If a worker's permission prompt asks for anything outside
the authorized list, ask the user before approving it.

## Preflight

Stop and report the first check that fails:
- `test "${HERDR_ENV:-}" = 1`. herdr control only works from inside a herdr pane.
- `command -v bun` succeeds.
- `gh auth status` and `herdr status` both succeed (herdr server is running).
- Each host in `--hosts` is on PATH (`claude`, `codex`, `cursor-agent`,
  `opencode`) and logged in.
- Jira epic: `jira.sh` is installed (toolu `jira` plugin) and authenticated.
  Linear epic: `LINEAR_API_KEY` is set (personal key or OAuth token).
- `gh api rate_limit` shows core above 1000 (`EPIC_GH_CORE_FLOOR`). Below it,
  wait for the reset: every worker's babysit spends the same token.
- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,
  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies (or the OpenCode-generated equivalents). Workers run as
  herdr agents and need those plugins installed in every host in `--hosts`
  (`npx @toolu/plugins install delivery-flow --host codex`, and so on).

## 1. Graph: what can run now

```bash
bun "$S/epic-graph.ts" <epic> --max N --save [--tracker T --repo O/R]  # table
bun "$S/epic-graph.ts" <epic> --max N --json --save                     # fields
```

For GitHub, the graph script reads the epic's native sub-issues. If there are
none, it falls back to task-list links in the epic body. Each sub-issue's
blocked-by relations come from GitHub's dependency API plus any "blocked by" or
"depends on" lines in the body. Jira and Linear children and blockers come
from their APIs (see Inputs). Every API call retries transient errors, waits
out rate limits (up to 15 min), and runs at most 4 at a time. It classifies every sub-issue as one of
`done`, `in_flight`, `ready`, `blocked` or `external_blocked`, computes waves,
counts how much downstream work each issue unblocks, finds a local checkout
per repo, and fills `launch_batch`. The batch is the ready issues that fit the
free slots, with the ones that unblock the most work first, so the critical
path starts immediately. State is written to `state_dir`
(`$EPIC_STATE_HOME` or the host default — Claude/Cursor Agent:
`~/.claude/epics/<owner>-<repo>-<n>/`, Codex: `$CODEX_HOME/toolu/epics/…`,
OpenCode: `$TOOLU_OPENCODE_HOME/toolu/epics/…`) and saved as `graph.json` there.

Show the user the table. Then:
- `complete: true` → go to step 6.
- `cycle` is non-empty → the dependency graph can't be scheduled. Report it
  and ask the user which edge is wrong.
- `external_blocked` issues wait on blockers outside the epic. Name the
  blockers in your report. Never launch these.
- `missing_checkouts` → the launcher clones those repos into `clone_root`.
  Tell the user.
- Before launching the batch, sanity-check it for hidden coupling. The graph
  only knows declared dependencies. If two batch issues' titles or bodies show
  they rewrite the same module, schema or migration, launch the higher-priority
  one now and hold the other for the next free slot, so they don't fight
  through rebases.

## 1b. Route: host, model, effort

```bash
bun "$S/route.ts" --graph <state_dir>/graph.json --hosts <spec>   # the launch batch
```

Jev scores each issue's complexity (title, labels, description excerpt,
dependency counts) into `trivial`, `standard`, `complex`, or `critical`. The
routing table maps the tier to a model and effort per host, e.g. Codex
`gpt-6-sol` low/medium/high/xhigh, Claude `sonnet` low/medium then `opus`
high/xhigh, Cursor `composer-2.5` up to `gpt-5.6-sol-xhigh`. OpenCode keeps
its configured model unless the table names one. Override the table in
`$EPIC_STATE_HOME/routing.json` (or `EPIC_ROUTING_FILE`):
`{"hosts": {"codex": [{"model": "gpt-6-sol", "effort": "low"}, …4 tiers]},
"prefer": {"critical": ["claude"]}}`.

Hosts fill by free capacity, so parallel issues spread across hosts (one on
Codex, the next on Claude). Hosts cooling down after a usage limit are
skipped. A `NONE` host means no capacity: leave that issue for the next slot.
Routes persist in `<state_dir>/routes/`; relaunches keep them. Show the user
the table. Without Jev (no `TYPESAFE_API_KEY`), say so; the tiers are then
heuristic.

**Dry run stops here.** For each batch issue, run
`bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref> --dry-run`.
It prints the exact clone, fetch, worktree, agent and prompt commands plus the
rendered worker brief, and executes nothing. Show the commands for every batch
issue, but the brief only once. Then summarize what happens after launch:
the worker pipeline, the merge policy, and which blocked issues each merge
unlocks. Dry-run writes nothing. With `--save` omitted, the graph can go to
`--out /tmp/...`.

## 2. Launch the batch

```bash
bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref> [--safe] [--kind K --model M --effort E]
```

Launch issues one at a time. For each issue the launcher:
1. clones the repo if it's missing
2. runs `git fetch origin <default>`
3. runs `herdr worktree create --branch feat/<n>-<slug> --base origin/<default>`,
   so each worker starts from a fresh main
4. runs `herdr agent start <key> --kind <host>` with the route's model and
   effort and the host's approval-bypass flags. A relaunch on the same host
   continues its last session (`--continue`, or `codex resume --last`).
   Refuses a first launch while the GitHub budget is under its floor.
5. renders `references/worker-brief.md` into `<state_dir>/briefs/<key>.md`,
   filling host-specific skill invocations and tracker-specific issue-read
   and PR-closing lines (the template's header comment lists them)
6. prompts the agent to follow the brief
7. records the issue in `<state_dir>/issues/<key>.json`

Re-running the launcher is safe: it reuses an open worktree and a live agent.
Workers report their phase to `<state_dir>/status/<key>.json` through
`report.ts`.

Tell the user which agents are running, on which host, model, and effort, and
that each one has a herdr workspace named after its key (`comemory-255`, …),
so they can watch or step in.

## 3. Watch (background)

```bash
bun "$S/epic-watch.ts" --state-dir <state_dir>        # Bash with run_in_background: true
```

Then end your turn. The watcher polls once a minute and costs nothing while
it waits. When it exits, you are re-invoked with JSON events. Keep exactly one
watcher running. After you handle the events, start it again.

| event | action |
|---|---|
| `ready` | Run the merge gate (step 4). |
| `needs-human` | Read the `note`. If the issue, epic or code answers it, send the answer with `herdr agent prompt <key> "<answer>"`. Otherwise ask the user and relay their answer. |
| `failed` | Read the note, then `herdr agent read <key> --source recent-unwrapped --lines 80`. Send a concrete new direction, or escalate to the user. |
| `blocked` | Read the agent's screen. If the pending approval is inside the authorization boundary, approve it with `herdr agent send-keys`. Otherwise ask the user. |
| `gone` | The watcher already snapshotted the worktree. Re-run `bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref>`. It resumes in the same worktree and session. After 2 relaunches, escalate. |
| `host-limited` | The host hit a provider usage or rate limit; the watcher put it on cooldown (60 min, `EPIC_HOST_COOLDOWN_MIN`). Move the issue: `bun "$S/route.ts" --graph <state_dir>/graph.json --hosts <spec> --issue <ref> --reroute`, then `launch-issue.ts … --issue <ref> --replace`. No free host: leave it; it resumes after the cooldown. `cooldown: false` means the issue record names no known host: fix its `kind` (or relaunch with `--kind`). |
| `gh-budget-low` | GitHub budget is under its floor. Launch nothing new, skip heartbeats' launches, and let rechecks wait (the watcher holds them) until `reset`. |
| `watcher-busy` | Another watcher owns this epic. Don't start a second; wait for its events. |
| `stalled` | `herdr agent prompt <key> "STATUS?" --wait --timeout 120000`, then read the reply. Nudge the worker or treat it as `failed`. |
| `recheck` | Re-run the merge gate for each listed key. |
| `heartbeat` | Re-run the graph (`--save`). This catches issues closed or reopened outside the run, and fills any free slot (route first). |

Every 15 minutes (`--checkpoint`) the watcher also snapshots each active
worktree to `refs/epic-wip/<key>` (see Guardrails). Snapshots it took are
listed under `checkpoints` in its output; no action needed.

For failures in herdr, git or gh, see `references/recovery.md`.

## 4. Merge gate

```bash
bun "$S/merge-gate.ts" --pr <repo>#<pr> --issue <ref> --epic <epic> --state-dir <state_dir> --key <key>
```

The gate checks, from GitHub itself:
- the PR is open and not a draft
- the head contains the tip of base (`behind_by == 0`), with no conflicts
- every check passes (a repo with no workflows counts as passing)
- there are zero unresolved review threads

The bot-verdict clearance comes from the worker's babysit, which is why the
gate runs only after `ready`.

| verdict | action |
|---|---|
| `merge` | Re-run the gate with `--merge`. It merges pinned to the verified head SHA (`--match-head-commit`), using the repo's allowed method (squash preferred), and retries with `--admin` only on a protection-only refusal. It then makes sure the issue is closed and ticks its line in the epic checklist. Continue to step 5. |
| `rebase` | The gate already disarmed any auto-merge (`auto_merge_disarmed`), so the rebased push cannot merge unverified; the same holds for `fix`. `herdr agent prompt <key> "REBASE"`. The worker rebases on the new main, re-runs the gate, force-pushes with lease, babysits, and reports `ready` again. |
| `fix` | `herdr agent prompt <key> "FIX: <reasons>"`. |
| `wait` | Only checks are pending. Re-run the gate with `--auto`: it arms GitHub auto-merge (`gh pr merge <n> --auto --squash --delete-branch --match-head-commit <sha>`), so GitHub merges the moment checks pass with no polling. The issue is `awaiting_merge`; the watcher's `recheck` (every 5 minutes) sees `merged` and settles the issue, or merges directly when the repo disallows auto-merge (`auto_merge: false`). |
| `merged` / `closed` | Something outside the run merged or closed it. Go to step 5 (merged), or ask the user (closed without merge). |

Merge one PR at a time per repo. Each merge moves main forward, so every other
open PR in that repo will get `rebase` at its own gate. This is how "rebase on
main before merge" holds for all of them without extra bookkeeping.

## 5. After each merge

```bash
bun "$S/finish-issue.ts" <state_dir> <key>
```

This exits the agent (babysit's cron ends with its session), snapshots the
worktree to `refs/epic-wip/<key>`, force-removes the worktree workspace
(leftover files are recorded first), deletes the local
branch and marks the record `merged`. Then go back to step 1: the merge may
have unblocked new issues. Launch into the free slots and restart the watcher.

## 6. Epic complete

The epic is complete when every sub-issue is closed (`complete: true`). Then:
1. `bun "$S/epic-close.ts" --graph <state_dir>/graph.json` posts the
   sub-issue → merged PR table (admin merges noted) and closes the epic in its
   tracker. `--dry-run` prints the table only. A Linear project stays open
   (`project-left-open`): tell the user to close it.
2. Stop the watcher and report to the user: PRs merged, admin merges, hosts
   and models used, anything that needed a human.

## Resume, status, stop

- **Resume.** Use the same invocation. The graph marks launched, unfinished
  issues as `in_flight`. For each one whose agent isn't live, re-run
  `bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref>`; it
  sends a resume prompt that continues from the last reported phase. Then start
  the watcher.
  The state dir is the source of truth. If you lose track (context
  compaction), re-run the graph and `bun "$S/epic-watch.ts" --state-dir <state_dir> --peek`.
- **Status.** Print the graph table plus
  `bun "$S/epic-watch.ts" --state-dir <state_dir> --peek` (per-issue stage,
  phase, PR and agent state). This consumes no events.
- **Stop.** Stop the watcher task only. Agents finish their current step and
  go idle. To tear down one issue, run
  `bun "$S/finish-issue.ts" <state_dir> <key> --abandon`, which keeps the branch.

## Guardrails

**Progress is never lost.**
- Workers commit and push their branch after each phase (the brief requires
  it) and report each phase before starting it.
- The watcher snapshots every active worktree, including uncommitted and
  untracked files, to `refs/epic-wip/<key>` in the shared repo, every 15
  minutes and whenever an agent vanishes. `finish-issue.ts` snapshots before
  removal and refuses `--abandon` over dirty work it could not snapshot.
  Restore: `git -C <checkout> checkout -b recover/<key> refs/epic-wip/<key>`
  (`git reflog refs/epic-wip/<key>` lists older snapshots).
- Relaunches continue the host's last session; a host switch starts fresh
  with the resume prompt (git log, status file, open PR).
- State files are written atomically; one watcher per epic (`watch.lock`).

**API budgets are respected.**
- GitHub, Jira, and Linear calls retry transient errors with backoff and
  honor `Retry-After` / reset headers; a reset over 15 min
  (`EPIC_API_MAX_SLEEP_S`) fails the call instead of hanging.
- Graph fetches run 4 at a time, below GitHub's secondary (burst) limit.
- New launches stop under the floor (`EPIC_GH_CORE_FLOOR` 1000,
  `EPIC_GH_GRAPHQL_FLOOR` 500); the watcher raises `gh-budget-low` and holds
  merge rechecks until the reset.
- Mutations (merge, close, comment) never retry a 5xx that may have applied.

## Keeping the orchestrator lean

- Don't read worker transcripts wholesale. Use status files, a one-line
  `STATUS?`, or `agent read --lines 80` when something is wrong.
- Trust script output. Don't re-fetch with ad-hoc `gh` calls what the graph,
  gate or watcher already reported.
- Don't micro-manage workers between phases. The brief already covers the
  pipeline, the rebase and fix messages, and when to escalate.
