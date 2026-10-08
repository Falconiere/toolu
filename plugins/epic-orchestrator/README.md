# epic-orchestrator

Drive a GitHub, Jira, or Linear epic to merged PRs. Builds the sub-issue
dependency graph, launches herdr worktrees with workers (delivery-flow from
brainstorm through PR and babysit), merges green PRs, and cleans up until the
epic is complete.

Works on **Claude Code**, **Codex**, **Cursor Agent**, and **OpenCode**, both
as the orchestrator and as workers. One run can mix hosts
(`--hosts claude:2,codex:2`): Jev scores each issue's complexity and a routing
table picks the model and effort per host.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies, in every worker host. Runtime: `bun`, `gh`, and `herdr`
(with `HERDR_ENV=1` inside a herdr pane).

Optional: `jev` with `TYPESAFE_API_KEY` (complexity routing; without it tiers
come from a heuristic), Jira environment credentials (Jira epics), and `LINEAR_API_KEY`
(Linear epics: a personal key `lin_api_…` is sent bare, an OAuth token
`lin_oauth_…` as `Bearer`; `LINEAR_API_URL` overrides the endpoint).

For Jira, export `JIRA_BASE_URL` and either `JIRA_PAT` or both `JIRA_EMAIL`
and `JIRA_API_TOKEN`. `JIRA_API_VERSION` accepts `2` or `3` and defaults to
`3`. The jira-cli config and keyring are not read. Workers read an item with
`bun --no-env-file scripts/jira-issue.ts get PAY-12` from the installed plugin.

### Claude Code

```text
/plugin install epic-orchestrator@toolu
```

### Codex

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

### Cursor Agent

Install the toolu marketplace plugin the same way as Claude Code (Cursor loads
the Claude-shaped plugin tree). Then invoke the `epic-orchestrator` skill or
the `/epic-orchestrator:epic` command.

### OpenCode

```bash
npx @toolu/plugins install epic-orchestrator --host opencode
```

That adds `@toolu/opencode` to OpenCode's config and enables
`epic-orchestrator` with its dependencies in the `toolu/plugins.json`
selection, for example:

```json
{ "version": 1, "enabled": ["toolu", "toolu-review", "pr-babysit", "brainstorm", "delivery-flow", "epic-orchestrator"] }
```

Wire OpenCode to the generated surface under
`tools/toolu-opencode/generated/` (shipped in `@toolu/opencode`); see
[docs/opencode.md](../../docs/opencode.md). The generated skill runs its
scripts from `$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR`, which the OpenCode
plugin's `shell.env` sets to the installed `plugins/epic-orchestrator`
directory while the plugin is enabled.

OpenCode has no background shell, so on OpenCode the orchestrator runs the
watcher in the foreground with `--max-wait 480` and runs it again until no
issue is active.

OpenCode workers need `opencode --version` 1.x on `PATH`; launch refuses
anything else before creating a worktree. Each worker is an
`opencode --auto --model provider/model` session in its own herdr worktree.
Its `task` calls reach agent-tier like any OpenCode session. Before the agent
starts, launch adds `/.opencode/toolu/state/` and `/.opencode/tmp/` to the
repository's shared `info/exclude`, so worker state never shows as a change,
is never captured by a `refs/epic-wip/*` snapshot, and never blocks worktree
removal. A worktree uses the plugin selection committed in its checkout
(`.opencode/toolu/plugins.json`), else your global one, else every installed
plugin; an uncommitted project selection does not reach worktrees.

## What it provides

- **Skill `epic-orchestrator` and command `epic`** — orchestrate an epic (or
  `status` / `stop`). Workers run the toolu delivery chain then babysit; the
  orchestrator merges and advances the graph.
- **Bun CLIs** under `scripts/` — `epic-graph.ts`, `jira-issue.ts`, `route.ts`,
  `launch-issue.ts`, `epic-watch.ts`, `merge-gate.ts`, `checkpoint.ts`,
  `epic-close.ts`, `finish-issue.ts`, `report.ts`. Tracker adapters live
  in `scripts/trackers/`.
- **Codex SessionStart** — warns when `delivery-flow` or its dependencies are missing.
- **SessionStart `engine-ensure`** — runs `toolu epic engine --ensure` when a native `toolu` is on `PATH`. An empty registry does nothing. A live engine is left alone.

## Native `toolu epic`

The resident engine is `toolu epic engine` (foreground). `--ensure` starts it only when `registry.json` lists an epic and the lock is free. `--replace` asks the current process to exit, then starts again.

| Verb | What it does |
| --- | --- |
| `engine [--replace] [--ensure]` | Resident process. One lock per resource root. |
| `start <state-dir>` | Register that epic directory and ensure the engine. |
| `status [epic]` | `engine`, `paused`, `epics`, `issues`, `attention`, GitHub budgets and PR check deadlines as one JSON document. |
| `pause` / `resume [epic]` | Hold or release effects. Reports still update snapshots. |
| `ack <key>` | Clear a stall attention item. |
| `answer <key> <text>` | Record an answer on the journal. |
| `wait [--max-seconds N]` | Block until a judgment, or print `{"state":"waiting"}`. |
| `report <phase> --status-file <file> [--pr N] [--note TEXT]` | Update one issue. Ensures the engine when the registry is non-empty. |
| `job <argv...>` | Run a command under the worktree's resource lease. The engine stays stopped. |
| `service install` | Write the user unit. It does not start systemd. |
| `token new` | Rotate the status bearer token in secrets.json. |
| `planned` | Lists `graph`, `route`, `launch`, `finish`, `close`, `release`, `jira`, `probe`, `gate`, and `queue` (#435, #448). |

### Fixed GitHub checks

The resident engine checks registered babysit PRs, their queued base branches,
the registered epic's sub-issues and open blockers every 180 seconds. A worker
report adds an immediate check while leaving the next scheduled slot in place.
The clock, REST `ETag`s and rate observations survive restart in `watch.json`.
An idle engine with no watched PR sends no GitHub requests.

Each PR check probes pull request state, head checks and status, comments and
reviews with conditional REST requests. An unchanged `304` costs no REST
primary point. The engine also calls the full `BabysitTick` for every watched
babysit PR; its GraphQL cost is separate from REST cost in the journal and in
`toolu epic status`. The production tick remains pending in #433, so its
current `NotPorted` result raises attention instead of claiming clearance.

`Retry-After` pauses GitHub checks, launches and merges for the requested
duration, then the fixed clock resumes. A low primary remaining budget pauses
launches and merges while checks continue. The native floors preserve the
existing watcher defaults of 1,000 REST and 500 GraphQL points. Status shows
each watched PR's last and next check times and separate budget counters. The
engine creates no webhook or GitHub listener.

## State directory

Override with `EPIC_STATE_HOME`. Otherwise:

| Host                       | Default                                                        |
| -------------------------- | -------------------------------------------------------------- |
| Claude Code / Cursor Agent | `~/.claude/epics/<owner>-<repo>-<n>/`                          |
| Codex                      | `$CODEX_HOME/toolu/epics/…` (default `~/.codex/toolu/epics/…`) |
| OpenCode                   | `$TOOLU_OPENCODE_HOME/toolu/epics/…` (or `$OPENCODE_HOME/…`)   |

## Trackers

| Tracker | Epic reference                                | Children                                      | Blockers                             | Done               |
| ------- | --------------------------------------------- | --------------------------------------------- | ------------------------------------ | ------------------ |
| GitHub  | URL, `owner/repo#N`, `#N`                     | sub-issues, else task-list links              | dependency API + "blocked by" text   | close              |
| Jira    | browse URL, `jira:KEY-1`, `KEY-1`             | `parent = KEY-1` (`"Epic Link"` on Server/DC) | "is blocked by" / "depends on" links | transition to Done |
| Linear  | issue or project URL, `linear:ENG-1`, `ENG-1` | sub-issues or project issues                  | `blocks` relations                   | completed state    |

Jira and Linear items map to a GitHub repo through a `repo:owner/name` label,
a `Repo: owner/name` description line, or `--repo` (default: current repo).

## Hosts and routing

| Host | Unattended flags | Model / effort | Resume |
|------|------------------|----------------|--------|
| Claude Code | `--dangerously-skip-permissions` | `--model` / `--effort` | `--resume <captured-id>` |
| Codex | `--no-daemon --dangerously-bypass-approvals-and-sandbox` | `--model` / `-c model_reasoning_effort=` | `resume <captured-id>` |
| Cursor Agent | `--yolo --trust --approve-mcps` | model id carries effort | `--resume <captured-id>` |
| OpenCode | `--auto` | `--model provider/model` (no effort flag) | `--session <captured-id>` |

`--safe` keeps approval prompts on. Default tiers (`trivial`, `standard`,
`complex`, `critical`) map to Claude `sonnet` low → `opus` xhigh, Codex
`gpt-6-sol` low → xhigh, Cursor `composer-2.5` → `gpt-5.6-sol-xhigh`, and
OpenCode's configured model. Override per host and tier in `routing.json`
under the state root, or point `EPIC_ROUTING_FILE` at one.

Routes are preferences, not reservations. Cached routes count against each batch
and are revalidated against the current pool; `NONE` refuses launch, including
`--force`. Launch reserves shared capacity before creating a workspace or
starting an agent. Start/prompt uncertainty retains ownership and requires
inspection before explicit `--reprompt` or `--replace`.
Replacement reserves its destination before stopping the source; both host slots
remain occupied until source shutdown is verified. Cleanup stages cannot be
reopened by a launch retry. Malformed saved routes, pools and ownership records
fail before launching work.
Explicit replacement cancels the source's owned jobs; pressure holds alone do
not cancel existing work.

## Machine resources and lifecycle

All host profiles and epic runs share `~/.local/state/toolu/resources`;
`TOOLU_RESOURCE_HOME` selects a different machine resource directory. Defaults
allow three agent sessions and one expensive job. Set positive integer limits
in `<resource-root>/policy.json`, for example:

```json
{ "maxAgents": 3, "maxJobs": 1, "hosts": { "claude": 1, "codex": 2 } }
```

Add `"pressure": false` to admit new work regardless of pressure holds on a host
where sampling is unreliable. Capacity limits still apply. Every process sharing the
resource root reads this file, so the override reaches the orchestrator and
every worker.

Launch writes a resource binding in the worktree's Git directory. Plan-ledger
checks acquire job capacity automatically. Run other expensive tests through
`bun <plugin>/scripts/job.ts -- <command> <args...>` from that worktree.
Admission refusal means wait for capacity and retry the same mandatory check.
A job runs for at most one hour so it cannot hold a shared slot forever; on
timeout its process group is stopped and `job.ts` exits 124.
Process groups retain their lease until background descendants exit. A crashed
job owner is reconciled only after its recorded group is gone; agent ownership
is never freed merely because its launcher exited.

Resource sampling runs at most once per 30 seconds. Load above 1.5 times effective
CPUs, less than 10% available memory, or Linux steal above 25% starts a pressure
window. Sixty seconds of pressure holds new work. Recovery requires 120 seconds
below 0.8 times effective CPUs, above 20% available memory and below 10% steal.
Existing useful jobs keep running. Available memory is `MemAvailable` on Linux
and free + inactive + speculative pages from `vm_stat` on macOS (Node's
`os.freemem()` there omits reclaimable cache), falling back to `os.freemem()`
when `vm_stat` fails or on other platforms. Non-Linux sampling uses load/memory
without claiming to measure steal. Hypervisor starvation is distinct from guest work.

Lifecycle operations verify host, pane and worktree. Shutdown distinguishes turn
cancellation from background jobs: Codex uses `--no-daemon` and `/stop` before `/exit`; OpenCode
uses the 1.x TUI, which runs its server in-process; ctrl+c exits it. Worktree
process inventories and process birth identities guard final cleanup. Missing shutdown/workspace evidence produces
`cleanup-incomplete`, preserves the lease and allows a later retry. Both merged
and abandoned work require a successful final snapshot before removal.
Prompt acknowledgement records blocked and provider-limited outcomes separately
from started/resumed work, and uncertain errors retain their native code and exit
status. Cleanup retries preserve successful removal and branch-deletion steps.
Removal intent is saved before deleting the workspace, so a crash before the
success record is written can be reconciled against Git and workload ownership.

Installed-host evidence is in `docs/toolu/evidence/epic-hosts*`; `probe.ts
--check-evidence` validates its recorded contract, not live provider availability.
The October 3 probe verified Codex and OpenCode tool/session lifecycles. Claude's
expired login and Cursor's locked keychain leave their tool/cancel coverage
explicitly unverified.

## Merging

The merge gate verifies the PR (rebased on base, checks green, zero
unresolved threads, babysit cleared) and merges pinned to the verified head.
When only checks are pending it arms GitHub auto-merge
(`gh pr merge <n> --auto --squash --delete-branch --match-head-commit <sha>`),
and disarms it whenever the PR needs a rebase or fix. Repos with auto-merge
disabled fall back to the watcher's periodic recheck.

## Guardrails

- **Progress:** workers commit and push each phase; the watcher snapshots
  every active worktree to `refs/epic-wip/<key>` every 15 minutes and when an
  agent disappears; teardown snapshots first; relaunches resume the host's
  captured session; one exclusive owner per epic. Unchanged snapshots reuse an
  independent Git index and compare tree/HEAD before creating a commit. The
  worker's real index is preserved. An interrupted private index write is
  recovered using a separate index generation.
- **Rate limits:** retries with backoff that honor `Retry-After` and reset
  headers (GitHub, Jira, Linear); at most 4 concurrent graph fetches; no new
  launches below `EPIC_GH_CORE_FLOOR` (1000) / `EPIC_GH_GRAPHQL_FLOOR` (500);
  hosts that hit a provider usage limit cool down (`EPIC_HOST_COOLDOWN_MIN`,
  60) and their issues move to another host.

Watcher deadlines, queued checkpoints and dependency backoff survive restarts
in `watch-state.json`. Urgent events precede optional checkpoints, which are
bounded and deduplicated by worktree. `EPIC_CHECKPOINT_BATCH` bounds each pass;
`EPIC_HERDR_BACKOFF_S` and `EPIC_HERDR_BACKOFF_MAX_S` bound dependency retries.
Dependency outages preserve existing stall acknowledgements and agent alerts.
Stale progress is distinct from a job heartbeat or UI `working` status. Alerts
repeat at `EPIC_STALL_REPEAT_MIN`; acknowledge with `epic-watch.ts --state-dir
<dir> --ack <key>`. Parked `ready`/`needs-human` workers stay quiet. `--peek`
consumes no events or persistent state.
