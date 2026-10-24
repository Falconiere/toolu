# epic-orchestrator

Drive a GitHub, Jira, or Linear epic to merged PRs. Builds the sub-issue
dependency graph, launches herdr worktrees with workers (delivery-flow from
brainstorm through PR and babysit), merges green PRs, and cleans up until the
epic is complete.

Works on **Claude Code**, **Codex**, **Cursor Agent**, and **Muse Code**, both
as the orchestrator and as workers. One run can mix hosts
(`--hosts claude:2,codex:2,muse:1`): Jev scores each issue's complexity and a
routing table picks the model and effort per host.

## Install

**Prerequisite:** the native `toolu` binary on `PATH`. See [docs/install.md](../../docs/install.md).

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies, in every worker host. Runtime: `toolu`, `gh`, and `herdr`
(with `HERDR_ENV=1` inside a herdr pane).

Optional: `jev` with `TYPESAFE_API_KEY` (complexity routing; without it tiers
come from a heuristic), Jira environment credentials (Jira epics), and `LINEAR_API_KEY`
(Linear epics: a personal key `lin_api_…` is sent bare, an OAuth token
`lin_oauth_…` as `Bearer`; `LINEAR_API_URL` overrides the endpoint).

For Jira, export `JIRA_BASE_URL` and either `JIRA_PAT` or both `JIRA_EMAIL`
and `JIRA_API_TOKEN`. `JIRA_API_VERSION` accepts `2` or `3` and defaults to
`3`. The jira-cli config and keyring are not read. Workers read an item with
`toolu epic jira issue get PAY-12`.

### Claude Code

```text
/plugin install epic-orchestrator@toolu
```

### Codex

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

### Cursor Agent

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host cursor
```

Cursor has no plugin install command, so this copies the plugins into
`~/.cursor/plugins/local` (see [the installer guide](../../docs/cli/installer.md#cursor));
`toolu --host cursor doctor` checks them. Then invoke the `epic-orchestrator`
skill or the `/epic-orchestrator:epic` command.

### Muse Code

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host muse --approve
```

Muse keeps a plugin's hooks inert until `muse plugins approve <name>` trusts
them, and asks again whenever a hook definition changes, for example after a
toolu upgrade. `--approve` approves them as part of the install. Without it,
`toolu --host muse doctor` reports `warn plugins: <name> capabilities need
approval; muse plugins approve <name>`, and the skill's preflight stops on
that line.

### Before a run

The skill's preflight checks each host in `--hosts`: its CLI and login, the
worker plugins (`toolu`, `toolu-review`, `pr-babysit`, `brainstorm`,
`delivery-flow`), and `toolu --host <host> doctor`. It prints the fix for the
first thing that fails.

The orchestrator needs the engine's socket, herdr's socket and the epic state
directory, so run it outside its host's sandbox. In the 2026-10-23 probe,
Codex's `workspace-write` sandbox and Muse's default sandbox blocked the
engine socket and every state path. Muse's also blocked herdr. Commands
fail with `Operation not permitted`. Start Codex with
`--sandbox danger-full-access` and Muse with `--disable-sandbox`, or approve
each escalation. Cursor Agent's sandbox reached them in the same probe; if
it doesn't, start it with `--sandbox disabled`. Claude Code runs commands
unsandboxed unless you turned its sandbox on.

The orchestrator waits for judgments with `toolu epic wait --epic <epic-key>`.
On Claude Code it runs as a background Bash call (45-minute default limit)
that wakes the session with a judgment or `waiting`, then starts again;
on Codex, Cursor Agent and Muse Code it runs as a foreground loop of
`toolu epic wait --epic <epic-key> --max-seconds 480`, since one tool call
there cannot block for hours. On Muse, a call that outlives its shell's yield
returns a session id, and its result arrives on its own. The engine does the
polling. `--epic` returns only that epic's items and engine-wide ones, so
several orchestrators can share one engine; an item handed out once is not
handed out again after an engine restart.

## What it provides

- **Skill `epic-orchestrator` and command `epic`** — preview and start an
  epic, then answer the engine's judgments; the command also wraps `start`,
  `status`, `pause`, `resume`, `stop` and `serve`. Workers run the toolu
  delivery chain through their PR, report `babysit` and go idle; the engine
  checks each PR every 3 minutes, prompts a worker only when something needs
  fixing, merges, and advances the graph.
- **The `toolu epic` namespace** — the resident engine and every action
  (graph, route, launch, gate, finish, close, release, Jira read, evidence
  probe) as native verbs; see [docs/cli/epic.md](../../docs/cli/epic.md). The
  plugin ships no scripts or bundles.
- **Native SessionStart hooks** (`toolu epic-orchestrator hook …`):
  `check-deps` warns on Codex when `delivery-flow` or its dependencies are
  missing; `check-binary` advises once per session when the agent's shell has
  no native `toolu`; `engine-ensure` starts the engine in process when an epic
  is registered and none runs, and reports a malformed `registry.json`.

## Native `toolu epic`

The resident engine is `toolu epic engine` (foreground). `--ensure` starts it only when `registry.json` lists an epic and the lock is free. `--replace` asks the current process to exit, then starts again.

| Verb | What it does |
| --- | --- |
| `engine [--replace] [--ensure]` | Resident process. One lock per resource root. |
| `start <state-dir> [--safe] [--no-jev]` | Register that epic directory, record the two flags in its `launch.json` (a start without a flag turns it off), and ensure the engine. `--safe` keeps host approval prompts on for its launches; `--no-jev` keeps routing and the coupling check heuristic. |
| `status [epic]` | `engine`, `paused`, `epics`, `issues`, `attention`, GitHub budgets and PR check deadlines as one JSON document. |
| `status --fleet` | This machine and every peer engine in `epic.peers` as the `/api/v1/fleet` document: per-machine state (`ok`, `unreachable`, `unauthorized`, `version-mismatch`), last seen, in-flight issues and hosts, and a summary. Peers are fetched by this command, never by the engine. |
| `pause` / `resume [epic]` | Hold or release effects. Reports still update snapshots. |
| `ack <key>` | Acknowledge the key's attention items (a keyless one by kind) and stop their reminders. |
| `answer <key> <text> [--by NAME]` | Send the text to the worker's prompt, then mark its items answered. Fails when no item is live or the prompt is not delivered. |
| `wait [--max-seconds N]` | Block until a judgment, or print `{"state":"waiting"}`. A `blocked` judgment carries the agent's screen as `pending`. |
| `report <phase> --status-file <file> [--pr N] [--note TEXT]` | Update one issue. Ensures the engine when the registry is non-empty. |
| `job <argv...>` | Run a command under the worktree's resource lease. The engine stays stopped. |
| `service install` | Write the user unit. It does not start systemd. |
| `token new` | Rotate the status bearer token in secrets.json. |
| `graph <epic>` | Build the dependency graph and the next launch batch; `--save` writes `graph.json`. |
| `route --graph <file>` | Score and route issues to hosts, models and efforts; saves routes and `pool.json`. |
| `launch --graph <file> --issue <ref>` | Launch or resume one issue in its herdr worktree (`--dry-run` plans only). |
| `gate --pr <ref>` | Verify a PR against the merge policy; `--merge` merges pinned to the head, `--auto` arms auto-merge. |
| `finish <state-dir> <key>` | Tear down an issue after its merge (`--abandon` keeps the branch). |
| `close --graph <file>` | Post the merged-PR summary and close a complete epic. |
| `release <key>` | Release a coupling hold so the issue launches next. |
| `jira issue get <key>` | Print one Jira issue as JSON. |
| `probe --check-evidence` | Validate the committed host-lifecycle evidence. |
| `queue [<epic>]` | Print each repository's merge queue in merge order (`--json` for the document). |

The engine runs launch, cleanup, checkpoint, the merge queue, the `STATUS?`
prompt, graph refresh and epic close in process; `TOOLU_EPIC_SCRIPTS`, a test
seam, replaces only merge, its reconcile and worker prompts (a deferring merge
script raises a `merge-ready` judgment for `toolu epic gate … --merge`).

### Merge queue

A `ready` worker, or a green babysit tick, queues its PR in its repository's
queue, ordered by the graph's `unblocks`, then by ready time, then by key.
Only the head is worked, on each of its PR checks and on a base move:

- behind its base, the engine rebases it with plain git in the worker's
  worktree, but only while the worker is parked at `ready` and no managed job
  runs there; the agent lease is fenced (`rebasing`), the worktree is
  snapshotted to `refs/epic-wip/<key>`, and the branch is pushed with
  `--force-with-lease`. A conflict is aborted, leaving the worktree as it was,
  and the worker gets `REBASE`;
- while its checks run, auto-merge is armed pinned to the head;
- green, it merges pinned to the verified SHA with the repository's method
  (squash preferred), with `--admin` only after a protection-only refusal; a
  push in between makes GitHub refuse, and the head is gated again;
- a failing check, an unresolved thread or a babysit finding sends it back to
  babysit, and any auto-merge the engine armed is disarmed.

The journal records each rebase round (`rebase`, `by` engine or worker); the
issue record keeps `queued_at` and `rebase_rounds`. `toolu epic queue` and
`/api/v1/queue` show the queue.

### Attention

Judgment events (`needs-human`, `failed`, `blocked`, `stall`, `babysit`, …)
become attention items. Each item is `open`, `acknowledged` or `answered`; the
key's next worker report closes it, and so does the engine once the item's
cause settles (a merge, a release, a finished cleanup, a good refresh). A returning stall or a new blocked spell
reopens a seen item. `toolu epic status`, `/api/v1/attention`
and the dashboard list every live item with its state and age. Items survive a
restart through the journal.

A new item shows a herdr notification (`epic.attention.herdr`, default on) and,
when `epic.attention.enabled` is true and `secrets.json` has a `notify_url`,
POSTs a JSON body with `text` to that URL: up to five attempts, retrying
timeouts, 429 and 5xx. An open item is announced again after
`epic.attention.remind_after_minutes` (default 30), then hourly. Every delivery
is journaled as `notify`, `notify-failed` or `notify-dropped`.

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
`toolu epic status`. The production tick is pr-babysit's (#433): a green tick
moves the issue to `ready` and queues its merge, a finding prompts the worker
with the tick result, and a failed tick raises a `babysit` attention item.

`Retry-After` pauses GitHub checks, launches and merges for the requested
duration, then the fixed clock resumes. A low primary remaining budget pauses
launches and merges while checks continue. The floors are 1,000 REST and 500
GraphQL points (`EPIC_GH_CORE_FLOOR`, `EPIC_GH_GRAPHQL_FLOOR`). Status shows
each watched PR's last and next check times and separate budget counters. The
engine creates no webhook or GitHub listener.

## State directory

Override with `EPIC_STATE_HOME`. Otherwise:

| Orchestrator environment                                         | Default                                                        |
| ---------------------------------------------------------------- | -------------------------------------------------------------- |
| Claude Code, Cursor Agent, Muse Code, and Codex                  | `~/.claude/epics/<owner>-<repo>-<n>/`                          |
| `CODEX_HOME` exported, no `CLAUDE_CONFIG_DIR` and no override    | `$CODEX_HOME/toolu/epics/…`                                    |
| `TOOLU_HOST_OVERRIDE=codex`                                      | `$CODEX_HOME/toolu/epics/…` (default `~/.codex/toolu/epics/…`) |

No host's shell tool sets `CODEX_HOME` or `TOOLU_HOST_OVERRIDE`, so every
orchestrator shares `~/.claude/epics` unless you export one of them. An epic
started under one of those variables resumes only with the same variable or
`EPIC_STATE_HOME`. Workers write their status files there on every host. Like
the engine socket, the directory must be writable from the orchestrator's
shell, outside the host sandbox (see [Before a run](#before-a-run)).

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
| Muse Code | `--yolo` | `--model` / `--reasoning-effort` | `resume <captured-id>` |

`--safe` keeps approval prompts on (Claude runs in its `auto` permission
mode, Muse in `--approval-mode on-request --trust-workspace`); pass it as `toolu epic start <state-dir> --safe`, which the engine and
a manual `toolu epic launch` read from `<state-dir>/launch.json` until a start
without it. With `--no-jev` the engine sends Jev nothing (workers' babysit
routing keeps its own `prBabysit.jev` setting): pass it to
`toolu epic route --graph <file> --hosts <spec> --no-jev` and to
`toolu epic start <state-dir> --no-jev`, which keeps the engine's own routing
and coupling check heuristic. `--kind`, `--model` and `--effort` override the route for
one manual `toolu epic launch`. Default tiers (`trivial`, `standard`,
`complex`, `critical`) map to Claude `sonnet` low → `opus` xhigh, Codex
`gpt-6-sol` low → xhigh, Cursor `composer-2.5` → `gpt-5.6-sol-xhigh`, Muse `muse-spark-1.3` low →
`muse-wonder-1.0` medium and high → `muse-wonder-full-1.0` xhigh. Override per host and tier in `routing.json`
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
`toolu epic job -- <command> <args...>` from that worktree.
Admission refusal means wait for capacity and retry the same mandatory check.
A job runs for at most one hour so it cannot hold a shared slot forever; on
timeout its process group is stopped and `toolu epic job` exits 124.
Process groups retain their lease until background descendants exit. A crashed
job owner is reconciled only after its recorded group is gone; agent ownership
is never freed merely because its launcher exited.

Resource sampling runs at most once per 30 seconds. Load above 1.5 times effective
CPUs, less than 10% available memory, or Linux steal above 25% starts a pressure
window. Sixty seconds of pressure holds new work. Recovery requires 120 seconds
below 0.8 times effective CPUs, above 20% available memory and below 10% steal.
Existing useful jobs keep running. Available memory is `MemAvailable` on Linux
and free + inactive + speculative pages from `vm_stat` on macOS (the free-page
count alone omits reclaimable cache), falling back to `sysctl`'s free page
count (`vm.page_free_count` times `hw.pagesize`) when `vm_stat` fails, and
never more than total memory. Non-Linux sampling uses load/memory
without claiming to measure steal. Hypervisor starvation is distinct from guest work.

Lifecycle operations verify host, pane and worktree. Shutdown distinguishes turn
cancellation from background jobs: Codex uses `--no-daemon` and `/stop` before `/exit`. Worktree
process inventories and process birth identities guard final cleanup. Shutdown waits up to
`EPIC_AGENT_EXIT_WAIT_S` (30) for herdr to stop listing the agent, then up to
`EPIC_AGENT_EXIT_GRACE_S` (10) for its worktree processes to exit. A teardown
whose agent or workload is still exiting (a `still exiting:` reason) is retried
by the engine up to `EPIC_CLEANUP_ATTEMPTS` (3) times, pausing
`EPIC_CLEANUP_BACKOFF_MS` (5000) and doubling. Any other failure is not retried.
Missing shutdown/workspace evidence produces
`cleanup-incomplete`, preserves the lease and allows a later retry. Both merged
and abandoned work require a successful final snapshot before removal.
Prompt acknowledgement records blocked and provider-limited outcomes separately
from started/resumed work, and uncertain errors retain their native code and exit
status. Cleanup retries preserve successful removal and branch-deletion steps.
Removal intent is saved before deleting the workspace, so a crash before the
success record is written can be reconciled against Git and workload ownership.

Installed-host evidence is in `docs/toolu/evidence/epic-hosts*`; `toolu epic
probe --check-evidence` validates its recorded contract, not live provider
availability.
The October 3 probe verified Codex tool/session lifecycles. Claude's
expired login and Cursor's locked keychain leave their tool/cancel coverage
explicitly unverified. The October 23 Muse probe (herdr 0.9.5, Muse Code 1.4.12)
verified all six capabilities. Muse writes its session at start under
`$XDG_DATA_HOME/muse/sessions/<yyyy>/<mm>/<dd>/<uuid>/` (default
`~/.local/share/muse`), where launch captures it. Teardown follows `/exit` with
`enter`, which confirms Muse's "Exit anyway" prompt when background work is
still running.

## Merging

The merge gate verifies the PR (rebased on base, checks green, zero
unresolved threads, babysit cleared) and merges pinned to the verified head.
When only checks are pending it arms GitHub auto-merge pinned to the verified
head, and disarms it whenever the PR needs a rebase or fix. The engine's
180-second PR check sees a merge, from the gate or from auto-merge, and cleans
up the issue.

## Guardrails

- **Progress:** workers commit and push each phase; the engine snapshots
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

The engine's PR check deadlines, budgets, checkpoint wave and herdr backoff
survive restarts in `watch.json`, and its journal lets a restart finish what
was in flight. A stalled worker gets one `STATUS?`, then a `stall` judgment;
`toolu epic ack <key>` restarts its clock. herdr outages back off from 30
seconds, doubling to 5 minutes. Parked `ready`/`needs-human` workers stay
quiet.

## Muse

Muse Code loads this plugin from `.muse-plugin/plugin.json`; its hook entries and `hooks/muse/` resolvers are generated by `cargo xtask print-muse-hook`. Validate with `muse plugins validate plugins/epic-orchestrator`.
