---
name: epic-orchestrator
description: Drive a GitHub, Jira, or Linear epic to done. Each open sub-issue gets its own herdr git worktree and agent (Claude Code, Codex, Cursor Agent, or OpenCode, routed by complexity), which runs delivery-flow from brainstorm through PR and pr-babysit. Independent issues run in parallel based on the dependency graph. Every branch is rebased on main, green PRs are merged automatically (--admin only when branch protection is the only thing blocking), and the epic closes when every sub-issue is closed. Use this whenever the user points at an epic, tracking issue, parent issue with sub-issues, or a checklist of linked issues and wants it worked, implemented, shipped, delivered, driven, resumed, or orchestrated. Examples — "work epic #248", "ship all the sub-issues of <url>", "pick up the replication epic", "which issues in this epic can run in parallel", "dry-run the epic", "where is the epic at". Not for a single standalone issue or PR.
---

# Epic orchestrator

You are the orchestrator. Your job is to turn one epic (GitHub, Jira, or
Linear) into merged GitHub PRs. Every open sub-issue gets its own herdr
worktree with an agent (the *worker*) inside it. Workers can run on Claude
Code, Codex, Cursor Agent, or OpenCode, several hosts at once. Workers write
the code and babysit their PRs. You gate each merge and make the calls the
engine cannot. Don't write product code yourself.

An epic run lasts hours, so keep your context small. The resident engine
(`toolu epic`) does the deterministic work: it launches each batch, checks
coupling, routes hosts, watches PRs, cleans up after merges, refreshes the
graph and closes the epic. It wakes you only for a judgment.

## Inputs

`<epic>` can be:
- GitHub: an issue URL, `owner/repo#N`, or `#N` (current repo).
- Jira: `https://<site>.atlassian.net/browse/KEY-12`, `jira:KEY-12`, or a bare
  `KEY-12` when only Jira is configured. Set `JIRA_BASE_URL` and either
  `JIRA_PAT` or both `JIRA_EMAIL` and `JIRA_API_TOKEN` in the environment.
  `JIRA_API_VERSION` is `2` or `3` (default `3`). Children are `parent = KEY-12` (or `"Epic Link"` on
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
- `--no-jev`: the engine's routing tiers and coupling check use heuristics
  and send it nothing (workers' babysit routing has its own Jev setting,
  `prBabysit.jev`). Pass it to `toolu epic route` (step 2) and to
  `toolu epic start <state_dir> --no-jev` (step 3), which records it in
  `<state_dir>/launch.json` for every issue the engine routes itself.
- `--safe`: keep each host's approval prompts on (Claude runs in its `auto`
  permission mode). Pass it as `toolu epic start <state_dir> --safe`; the
  engine and a manual `toolu epic launch` read it from `launch.json`.
  Default is unattended: `--dangerously-skip-permissions` (Claude),
  `--dangerously-bypass-approvals-and-sandbox` (Codex),
  `--yolo --trust --approve-mcps` (Cursor), `--auto` (OpenCode).
  Every `start` rewrites `launch.json`, so a start without `--safe` or
  `--no-jev` turns it off; repeat the flags when resuming.
- `--kind`, `--model`, `--effort` override the route for one manual
  `toolu epic launch` (they are that verb's flags, not `toolu epic route`'s).
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
`toolu epic gate` enforces that rule. `--admin` never overrides a failing
check, a pending check, or an unresolved thread.

Not authorized: pushing to `main`, or touching PRs, branches or worktrees
outside the epic. If a worker's permission prompt asks for anything outside
the authorized list, ask the user before approving it.

## Preflight

Stop and report the first check that fails:
- `test "${HERDR_ENV:-}" = 1`. herdr control only works from inside a herdr pane.
- `toolu --version` prints a version (the native binary; see `docs/install.md`).
- `gh auth status` and `herdr status` both succeed (herdr server is running).
- Each host in `--hosts` is on PATH (`claude`, `codex`, `cursor-agent`,
  `opencode`) and logged in. For OpenCode, `opencode --version` must be 1.x
  (toolu's OpenCode plugin targets opencode-ai 1.x).
- Jira epic: `JIRA_BASE_URL` and either `JIRA_PAT` or both `JIRA_EMAIL` and
  `JIRA_API_TOKEN` are exported. The jira-cli config and keyring are not read.
  Linear epic: `LINEAR_API_KEY` is set (personal key or OAuth token).
- `gh api rate_limit` shows core above 1000 (`EPIC_GH_CORE_FLOOR`). Below it,
  wait for the reset: every worker's babysit spends the same token.
- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,
  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies (or the OpenCode-generated equivalents). Workers run as
  herdr agents and need those plugins installed in every host in `--hosts`
  (`npx @toolu/plugins install delivery-flow --host codex`, and so on).
  An OpenCode worker uses its worktree's selection: a committed
  `.opencode/toolu/plugins.json`, else the global one, else every installed
  plugin. An uncommitted project selection does not reach worktrees.

## 1. Graph: what can run now

```bash
toolu epic graph <epic> --max N --save    # table; add --tracker T --repo O/R for Jira or Linear
toolu --json epic graph <epic> --max N --save    # the graph.json document
```

For a dry run, replace `--save` with `--dry-run`. The verb reads the tracker
and writes nothing, and `--out` writes nothing under `--dry-run` either, so
keep the document for the next steps in a temp file with the shell:

```bash
toolu --json epic graph <epic> --max N --dry-run > /tmp/epic-graph.json
```

For GitHub, the graph reads the epic's native sub-issues. If there are none,
it falls back to task-list links in the epic body. Each sub-issue's
blocked-by relations come from GitHub's dependency API plus any "blocked by" or
"depends on" lines in the body. Jira and Linear children and blockers come
from their APIs (see Inputs). Every API call retries transient errors, waits
out rate limits (up to 15 min), and runs at most 4 at a time. It classifies every sub-issue as one of
`done`, `in_flight`, `ready`, `blocked` or `external_blocked`, computes waves,
counts how much downstream work each issue unblocks, finds a local checkout
per repo, and fills `launch_batch`. State is written to `state_dir`
(`$EPIC_STATE_HOME` or the host default — Claude/Cursor Agent:
`~/.claude/epics/<owner>-<repo>-<n>/`, Codex: `$CODEX_HOME/toolu/epics/…`,
OpenCode: `$TOOLU_OPENCODE_HOME/toolu/epics/…`) and saved as `graph.json` there.

Show the user the table. Then:
- `complete: true` → go to step 5.
- `cycle` is non-empty → the dependency graph can't be scheduled. Report it
  and ask the user which edge is wrong.
- `external_blocked` issues wait on blockers outside the epic. Name the
  blockers in your report. The engine never launches these.
- `missing_checkouts` → the launcher clones those repos into `clone_root`.
  Tell the user.

## 2. Route: host, model, effort

Not in a dry run (it has no `<state_dir>/graph.json`; see the end of this
step):

```bash
toolu epic route --graph <state_dir>/graph.json --hosts <spec>   # saves the routes and the host pool
# add --issue <ref> to route one issue, --no-jev for the heuristic tiers
```

Jev scores each issue's complexity (title, labels, description excerpt,
dependency counts) into `trivial`, `standard`, `complex`, or `critical`. The
routing table maps the tier to a model and effort per host, e.g. Codex
`gpt-6-sol` low/medium/high/xhigh, Claude `sonnet` low/medium then `opus`
high/xhigh, Cursor `composer-2.5` up to `gpt-5.6-sol-xhigh`. OpenCode keeps
its configured model unless the table names one (`provider/model`). Override the table in
`$EPIC_STATE_HOME/routing.json` (or `EPIC_ROUTING_FILE`):
`{"hosts": {"codex": [{"model": "gpt-6-sol", "effort": "low"}, …4 tiers]},
"prefer": {"critical": ["claude"]}}`.

Hosts fill by free capacity, so parallel issues spread across hosts. Hosts
cooling down after a usage limit are skipped. A `NONE` host means no capacity
right now. The routes persist in `<state_dir>/routes/` and the pool in
`<state_dir>/pool.json`; the engine keeps a valid saved route and routes the
rest itself. Show the user the table. Without Jev (no `TYPESAFE_API_KEY`),
say so; the tiers are then heuristic.

**Dry run stops here, and writes nothing.** Use the graph file from step 1's
dry run, route it without saving, then plan each batch issue:

```bash
toolu epic route --graph /tmp/epic-graph.json --hosts <spec> --dry-run
toolu epic launch --graph /tmp/epic-graph.json --issue <ref> --dry-run --kind <host> --model <model> --effort <effort>
```

The unsaved route reaches the launch only through `--kind`, `--model` and
`--effort`: copy them from the route table for each issue (drop `--model` or
`--effort` where the table shows none), and add `--safe` when the user passed
it, since a dry run has no `launch.json`. The launch prints the exact clone, fetch, worktree, agent and prompt commands
plus the rendered worker brief, and executes nothing. Show the commands for
every batch issue, but the brief only once. Then summarize what happens after
launch: the worker pipeline, the merge policy, and which blocked issues each
merge unlocks.

## 3. Start the engine

```bash
toolu epic start <state_dir>
toolu epic start <state_dir> --safe --no-jev    # instead, with the options the user passed
```

This registers the epic, records `--safe` and `--no-jev` in
`<state_dir>/launch.json`, and starts the resident engine (once per machine;
a later `start` registers and rewrites the settings, so a start without a
flag turns it off). `--safe` keeps the host approval prompts on for every
launch the engine makes; `--no-jev` keeps its routing and coupling check
heuristic. From then on the engine, for this epic:
1. forms a batch from the ready issues that fit the free slots, most
   unblocking first, and launches it one issue at a time
2. compares each launch with the batch members ahead of it: two issues that
   change the same module, schema or migration (Jev, else shared paths and
   migration names) hold the later one with a `coupling-hold` judgment
3. routes it and launches it in process: clone, `git fetch`, `herdr worktree
   create` from a fresh `origin/<default>`, `herdr agent start` with the
   route's model and effort, the brief rendered into `<state_dir>/briefs/`,
   and the record in `<state_dir>/issues/<key>.json`
4. snapshots every active worktree to `refs/epic-wip/<key>` every 15 minutes,
   sends one `STATUS?` to a stalled worker, relaunches a lost agent in its
   worktree and session (twice), and moves a rate-limited worker to a free
   host
5. watches each PR, cleans up after its merge (`toolu epic finish`, then
   closes and ticks the sub-issue), refreshes the graph, and launches what the
   merge unblocked
6. closes the epic once every sub-issue is done

Workers report their phase through `toolu epic report`. Tell the user which
agents are running, on which host, model, and effort (`toolu epic status`),
and that each one has a herdr workspace named after its key (`comemory-255`,
…), so they can watch or step in.

## 4. Wait for judgments

```bash
toolu --json epic wait --max-seconds 480    # Bash tool with a 600000 ms timeout
```

It returns one judgment as soon as one is pending: `kind`, `key`, `epic`,
`note`, `state`, `age_ms`, and for `blocked` also `pending` (the agent's
screen). Else it returns `{"state": "waiting"}` when the limit passes. Handle
it, then run it again in the same turn, and keep that loop going while any
issue is active. The engine has already notified herdr and the configured
URL, and reminds until the item is acknowledged or answered; a worker report
closes its key's items, and the engine closes an item once its cause settles
(a merge, a release, a finished cleanup, a good refresh). Ack the rest with
`toolu epic ack <key>` once handled. `toolu epic answer <key> "<text>"` prompts the worker
with the text and records the answer.

| kind | action |
|---|---|
| `merge-ready` | The worker reported `ready`. Run the merge gate (below); the merge closes the item. |
| `needs-human` | Read the `note`. If the issue, epic or code answers it, send it with `toolu epic answer <key> "<answer>"`. Otherwise ask the user and relay their answer the same way. |
| `failed` | Read the note, then `herdr agent read <key> --source recent-unwrapped --lines 80`. Send a concrete new direction with `toolu epic answer <key> "<direction>"`, or escalate to the user. An `uncertain launch` note means the start may have happened: inspect the record and the pane, then `toolu epic launch --graph <state_dir>/graph.json --issue <ref> --reprompt` (a verified session) or `--replace`. `epic close uncertain` means the epic may be closed already: check it in the tracker; never close it twice. |
| `blocked` | Read `pending`. If the approval is inside the authorization boundary, approve it with `herdr agent send-keys`. Otherwise ask the user. Then `toolu epic ack <key>`. |
| `stall` | The engine already sent `STATUS?`. Read the reply (`herdr agent read <key> --lines 40`), then nudge the worker with `toolu epic answer <key> "<direction>"`, treat it as `failed`, or `toolu epic ack <key>` to wait one more stall length. |
| `relaunch-limit` | The agent vanished a third time. Escalate to the user, then `toolu epic ack <key>`. |
| `coupling-hold` | The issue overlaps the batch member in the note. When that member merges (or you judge the overlap false), `toolu epic release <key>`, which closes the item; the issue launches next without the check. |
| `cleanup-incomplete` | The teardown refused (see the note and `references/recovery.md`). Fix the cause, then `toolu epic finish <state_dir> <key>` and `toolu epic ack <key>`. |
| `refresh-failed` | The tracker failed three times in a row (often credentials). Fix it; the engine keeps retrying every 3 minutes and closes the item after a good refresh. |
| `herdr-error` | herdr did not answer. Run `herdr status`. The engine backs off 30 seconds, doubling to 5 minutes, and recovers on its own once herdr answers; then `toolu epic ack <key>`. Escalate to the user if it persists. |
| `reopened` | A merged sub-issue was reopened. Read the reopening comment; a regression usually deserves a fresh branch: `toolu epic launch --graph <state_dir>/graph.json --issue <ref> --force`. Then `toolu epic ack <key>`. |
| `pr-closed` | The PR was closed without merge. Ask the user, then `toolu epic ack <key>`. |
| `babysit` | The PR check needs attention (the note says why). Prompt the worker with `toolu epic answer <key> "<direction>"` or escalate, then `toolu epic ack <key>`. |
| `dependency-cycle` | Ask the user which edge is wrong, fix the graph, then `toolu epic ack dependency-cycle`. |
| `epic-complete` | `toolu epic ack <key>` (the epic key), then go to step 5. |

**Merge gate.**

```bash
toolu epic gate --pr <repo>#<pr> --issue <ref> --epic <epic> --state-dir <state_dir> --key <key>
```

The gate checks, from GitHub itself: the PR is open and not a draft, the head
contains the tip of base with no conflicts, every check passes (a repo with no
workflows counts as passing), and there are zero unresolved review threads.
The bot-verdict clearance comes from the worker's babysit, which is why the
gate runs only after `ready`.

| verdict | action |
|---|---|
| `merge` | Re-run the gate with `--merge`. It merges pinned to the verified head SHA, using the repo's allowed method (squash preferred), retries with `--admin` only on a protection-only refusal, closes the sub-issue and ticks the epic. The engine sees the merge at once and cleans up. |
| `rebase` | The gate already disarmed any auto-merge, so the rebased push cannot merge unverified; the same holds for `fix`. `herdr agent prompt <key> "REBASE"`. The worker rebases on the new main, re-runs its gate, force-pushes with lease, babysits, and reports `ready` again. |
| `fix` | `herdr agent prompt <key> "FIX: <reasons>"`. |
| `wait` | Only checks are pending. Re-run the gate with `--auto`: it arms GitHub auto-merge pinned to the head, so GitHub merges the moment checks pass. The engine's PR check sees the merge and cleans up. |
| `merged` / `closed` | Something outside the run merged or closed it. The engine cleans up a merge; ask the user about a close without merge. |

Merge one PR at a time per repo. Each merge moves main forward, so every other
open PR in that repo will get `rebase` at its own gate. This is how "rebase on
main before merge" holds for all of them without extra bookkeeping.

For failures in herdr, git or gh, see `references/recovery.md`.

## 5. Epic complete

The engine closes the epic when every sub-issue is closed and its cleanup has
finished: it posts the sub-issue → merged PR table (admin merges noted) and
raises `epic-complete`. A Linear project stays open (`project-left-open`):
tell the user to close it. To close by hand, `toolu epic close --graph
<state_dir>/graph.json` (`--dry-run` prints the table only).

Report to the user: PRs merged, admin merges, hosts and models used, anything
that needed a human.

## Resume, status, stop

- **Resume.** Use the same invocation. Re-run the graph, then
  `toolu epic start <state_dir>` (add `--safe` and `--no-jev` if the run used them): the engine
  reloads its journal and records, resumes every in-flight action, and
  relaunches nothing whose start is uncertain. The state dir is the source of truth. If you lose track (context
  compaction), run `toolu --json epic status`.
- **Status.** Print the graph table plus `toolu --json epic status` (per-issue
  stage, phase, PR and agent state). For one epic or worker, run
  `toolu --json epic status <key>`. The dashboard at
  `http://127.0.0.1:7717/` follows the journal without consuming judgments.
- **Stop.** `toolu epic pause <epic-key>` stops new launches, merges and
  cleanups; running agents finish their current step and go idle (`toolu epic
  resume <epic-key>` continues). The `<epic-key>` is the basename of
  `<state_dir>` (for example `falconiere-toolu-402`), not the reference you
  passed to the graph. To tear down one issue, run
  `toolu epic finish <state_dir> <key> --abandon`, which keeps the branch.

## Guardrails

**Progress is never lost.**
- Workers commit and push their branch after each phase (the brief requires
  it) and report each phase before starting it.
- The engine snapshots every active worktree, including uncommitted and
  untracked files, to `refs/epic-wip/<key>` in the shared repo, every 15
  minutes and whenever an agent vanishes. `toolu epic finish` snapshots before
  removal and refuses every cleanup over work it could not snapshot.
  Restore: `git -C <checkout> checkout -b recover/<key> refs/epic-wip/<key>`
  (`git reflog refs/epic-wip/<key>` lists older snapshots).
- Relaunches use captured session IDs. Uncertain start/prompt outcomes retain
  capacity; inspect before explicit `--reprompt` or `--replace`. A host switch
  verifies old workload exit before starting fresh with the resume prompt.
- Machine-wide agent/job leases and shared cooldowns cover every epic and host
  profile; `--force` cannot bypass capacity. Ledger checks acquire job leases
  automatically. See the README's machine resource policy.
- The engine journals every action's intent before it acts; after a restart
  it finishes what was in flight, never starts a second agent for an
  uncertain launch, and never comments twice on an epic.

**API budgets are respected.**
- GitHub, Jira, and Linear calls retry transient errors with backoff and
  honor `Retry-After` / reset headers; a reset over 15 min
  (`EPIC_API_MAX_SLEEP_S`) fails the call instead of hanging, and one engine
  effect never sleeps longer than that in total.
- Graph fetches run 4 at a time, below GitHub's secondary (burst) limit.
- New launches and merges wait under the floor (`EPIC_GH_CORE_FLOOR` 1000,
  `EPIC_GH_GRAPHQL_FLOOR` 500) until the reset.
- Mutations (merge, close, comment) never retry a 5xx that may have applied.

## Keeping the orchestrator lean

- Don't read worker transcripts wholesale. Use status files, a one-line
  `STATUS?`, or `agent read --lines 80` when something is wrong.
- Trust `toolu epic` output. Don't re-fetch with ad-hoc `gh` calls what the
  graph, gate or engine already reported.
- Don't micro-manage workers between phases. The brief already covers the
  pipeline, the rebase and fix messages, and when to escalate.
