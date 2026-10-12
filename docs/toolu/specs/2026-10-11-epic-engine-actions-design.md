# Epic engine actions: graph, route, launch, finish, close, trackers — Design

**Date:** 2026-10-11   **Status:** Approved   **Author:** Claude (epic worker, #435)   **Topic:** Port epic-orchestrator's deterministic actions to `crates/epic-orchestrator` and let the resident engine run them.

## Problem

The resident engine (#434, #446, #447, #449) journals transitions, watches herdr
and GitHub, and answers `wait`, but every action it arms is still a script it
cannot run: launch, cleanup, prompt and reconcile go to `TOOLU_EPIC_SCRIPTS` or
are journaled `deferred`. The orchestrator LLM therefore still runs
`plugins/epic-orchestrator/scripts/*.ts` by hand (graph, route, launch,
merge-gate, finish, close), and those scripts need Bun. Issue #435 ports the
actions to Rust, lets the engine run them on its own, and deletes `scripts/`
and `hooks/src`.

## Non-Goals

1. The merge queue, mechanical rebase, auto-merge driven by the engine, and
   unattended merge (#448). The engine's `merge` action keeps the
   `TOOLU_EPIC_SCRIPTS/merge` seam (and its merge `reconcile`) for the scripted
   acceptance; without it the merge is deferred and raises `merge-ready`.
2. The babysit tick (#433). `BabysitTick` stays as wired.
3. The skill redesign, size budget, per-host wait loops and evals (#452). This
   issue swaps commands and keeps the skill's structure.
4. Removing Bun from other plugins or the launcher's Bun fallback (#440).
5. A write API over HTTP, webhooks, or a change to the authorization boundary.

**Scope transfer recorded here (not an edit of #448):** deleting `scripts/`
removes `merge-gate.ts`, the orchestrator's only merge gate. To keep the
product working between #435 and #448, this issue ports the gate's verdicts,
merge, auto-merge arm/disarm and settle as the manual `toolu epic gate` verb
(Jev 0.80 over deferring entirely). The engine does not call it. #448 keeps
the queue, the mechanical rebase, the engine's own merge (which will call this
verb's library), and `toolu epic queue`; its "merge-gate tests pass against
`toolu epic gate`" item is delivered here and #448 inherits it. The PR body
states this transfer and links #448; commenting on #448 itself is outside
this worker's authorization, so the orchestrator relays it if needed.

## Architecture

**Decision.** Port each script as a library module in `crates/epic-orchestrator`,
expose it as a `toolu epic` verb, and call the same functions from the engine
on one dedicated effect thread. The decisive trade-off: a launch can take three
minutes (clone, `agent start` 90 s, prompt wait 60 s, session capture 10 s), so
effects leave the state thread, which keeps answering reports, `wait`, status
and herdr events. Effects still run one at a time, as launches do today.

Reused: `herdr::Client` (socket first, CLI for newer protocols), `journal`,
`commit_effect`, `logic`/`schedule`/`recover`, `watch`/`github_engine`, `limit`,
`toolu_engine::resources`, `toolu_runtime::process`,
`toolu_runtime::startup::{native_toolu_advice, codex_dependency_notice}`,
`toolu-github` (`Config::scheduled()`, one attempt per call), `toolu-http`,
`toolu-jev-client`, `toolu_state::time`.

### Modules

Every new file stays under 300 code lines; directories hold one module each
(`<name>.rs` plus `<name>/`), tests in `<dir>/tests/<module>_test.rs`.

| Module (new unless noted) | Port of | Contents |
|---|---|---|
| `refs` | `common.ts` | `parse_ref`, `ISSUE_REF`, `issue_key`, `slugify`, `key_for`, `branch_for`, `epics_home` (`EPIC_STATE_HOME`, host homes) |
| `retry` | `ratelimit.ts` | `classify`, `Policy::from_env` (`EPIC_API_ATTEMPTS` 5, `EPIC_API_BACKOFF_MS` 2000, `EPIC_API_MAX_SLEEP_S` 900), `with_retry(policy, write, call)`, `reset_from_headers`; error mapping below |
| `tracker` + `tracker/{types,detect,github,github_body,jira,jira_client,jira_parse,linear}` | `trackers/*` | `Tracker` trait: `epic`, `children`, `is_closed`, `close_issue`, `tick_epic`, `close_epic`, `state_slug` |
| `graph` + `graph/{levels,classify,batch,build,checkouts,render}` | `epic-graph.ts`, `checkouts.ts` | levels, downstream, chain, `classify`, `pick_batch`, cycle, checkouts, table, `graph.json` |
| `hosts` | `hosts.ts` | kinds and aliases, `agent_args`, `skill_ref`, OpenCode model/version checks, OpenCode skill ids |
| `route` + `route/{table,tier,score,pool,assign,render}` | `route.ts` | tiers, Jev scores in process, heuristic tiers, routing table, pool, cooldowns, capacity, persisted routes |
| `launch` + `launch/{record,brief,checkout,worktree,agent,session,lifecycle,admission,opencode,failure}` | `launch-*.ts`, `launch/agent.ts`, `session.ts`, `lifecycle.ts`, `admission.ts`, `opencode-worker.ts` | the launch, its dry-run log, and its records |
| `workload` + `workload/{linux,mac}` | `workload.ts` | worktree-owned process inventory, `verify_owned_exit` |
| `checkpoint` (rewritten) | `checkpoint.ts` | retained private index, unpushed pinning, unchanged-tree dedup, HEAD-change retry, lock, `Snapshot` |
| `finish` + `finish/{record,ownership,remove,branch}` | `finish-issue.ts`, `finish-state.ts` | strict record, legacy fence adoption, removal intent, live/removed/crash paths, lease release |
| `close` | `epic-close.ts` | summary table, tracker close |
| `gate` + `gate/{assess,merge,auto,settle}` | `merge-gate.ts` | verdicts, merge pinned to head, admin retry, auto-merge, settle, `last_gate` |
| `coupling` | skill step 1 | Jev pair scores, heuristic |
| `jira_cli`, `probe_evidence` | `jira-issue.ts`, `probe.ts` | read-only verbs |
| `hook` | `check-binary.ts`, `check-deps.ts`, `engine-ensure.ts` | `check_binary`, `check_deps`, `engine_ensure` → `Outcome` |
| `verbs_actions` | — | clap builders and dispatch of the new verbs (split per verb where a builder nears 50 lines) |
| `executor`, `effect_thread`, `effects_run` | — | `Executor`, the effect thread, in-process action bodies |
| `refill`, `holds`, `epics`, `refresh` | — | batch selection, coupling holds, per-epic state, graph refresh in the engine |
| `dispatch_ops` | — | socket ops `release`, `event merged` (keeps `dispatch.rs` under 300) |
| `socket_loop` | — | the state loop and `Msg`, moved out of `socket.rs` to admit `Msg::Effect` |

`effects.rs` keeps only the merge and merge-reconcile script seam.

`toolu_engine::resources` gains `lease-transitions.ts`
(`prepare_agent_migration`, `migrate_agent_lease`, `cancel_agent_migration`,
`fence_worktree`, `WorktreeJobsActive`), `active_jobs`, and the `worktree`,
`pane`, `session` fields of `LeasePatch`, with TypeScript's error texts.

### Calls into herdr

`herdr::Call` gains, each with its socket method and CLI argv:

| Call | Socket method and params | CLI form |
|---|---|---|
| `WorktreeList { cwd }` | `worktree.list {cwd}` | `herdr worktree list --cwd C` |
| `WorktreeOpen { cwd, path, label }` | `worktree.open {cwd, path, label, focus:false}` | `herdr worktree open --cwd C --path P --label L --no-focus` |
| `Create` (existing) | adds `focus:false` | adds `--no-focus` |
| `PaneList { workspace }` | `pane.list {workspace_id}` | `herdr pane list --workspace W` |
| `PaneInfo { pane }` | `pane.process_info {pane_id}` | `herdr pane process-info --pane P` |
| `AgentList` | `agent.list {}` | `herdr agent list` |
| `SendKeys { target, keys }` | `agent.send_keys {target, keys}` | `herdr agent send-keys T K…` |
| `Wait { target, until, timeout_ms }` | `agent.wait {target, until, timeout_ms}` | `herdr agent wait T --until … --timeout N` |
| `PromptWait { target, text, until, timeout_ms }` | `agent.prompt {target, text, wait:{until, timeout_ms}}` | `herdr agent prompt T X --wait --until … --timeout N` |
| `WorkspaceList` | `workspace.list {}` | `herdr workspace list` |
| `Read` (existing) | `agent.read` 15 lines | unchanged |

A herdr error object, non-JSON output or a timeout is a `CommandError` with the
herdr code; `timeout` and `agent_prompt_stalled` are uncertain, as in
`common.ts`. Dry-run logs print the CLI form.

### Trackers, retries and credentials

- GitHub uses `toolu_github::Client` with `Config::scheduled()` (one attempt;
  a rate limit is returned with its wait). Jira and Linear use `toolu-http`.
  Every call goes through `retry::with_retry(policy, write, call)`.
- Error mapping: `toolu_github::Error::RateLimited` and HTTP 429/`RATELIMITED`
  → rate limit with its reset (`retry-after`, `x-ratelimit-reset` when
  remaining is 0, Linear epoch-ms); 5xx, transport and timeout → transient;
  every other status, decode, GraphQL error, config and token error →
  permanent. Writes (`POST`, `PATCH`, `PUT`, `DELETE`, GraphQL mutations,
  merges) never retry a transient. A reset beyond `EPIC_API_MAX_SLEEP_S` fails
  with the reset time instead of sleeping.
- The engine's effects use the same policy (Jev 0.95 for parity over a
  no-sleep or shortened policy): the effect thread may sleep up to the cap;
  the state thread does not.
- Credentials come from the process environment: `GH_TOKEN` or `gh auth
  token` (`toolu-github`), `JIRA_BASE_URL` + `JIRA_PAT` or `JIRA_EMAIL` +
  `JIRA_API_TOKEN` (+ `JIRA_API_VERSION`), `LINEAR_API_KEY` (+
  `LINEAR_API_URL`), `TYPESAFE_API_KEY` for Jev. The resident engine inherits
  the environment of the command that started it (`toolu epic start`,
  `engine --ensure`). `secrets.json` (#463) holds none of them. No credential
  is written to disk, journaled, or printed; errors redact them.

### Engine changes

- **Effects.** `Step::Call` goes to an `Executor`. `Inline` (tests, one-shot)
  runs the call at once. `Thread` sends it to the effect thread, which holds at
  most one call; the state thread marks the pending action `running` in memory
  so `pending_step` never issues it twice, and issues the next call only after
  the outcome returns as `Msg::Effect { key, action, token, outcome }`.
  - Calls never block the outbox. A pending issue action (launch, cleanup,
    checkpoint, merge) stays on its issue and is issued only when the executor
    is idle; an epic action (refresh, close) likewise stays on its epic. A
    `Step::Call` taken from the outbox while the executor is busy (the
    `STATUS?` prompt) moves to a FIFO `parked` queue, and the `Journal`,
    `Status` and `Issue` steps behind it keep running at once. Parked calls go
    first when the executor frees.
  - A call that has not started is not started while its epic is paused or
    the engine drains; a running call finishes (a launch cannot be abandoned
    halfway).
  - Each effect is bounded by its own sub-call deadlines (herdr 10–120 s per
    call as in TypeScript, git 30 s, clone 120 s) and, for tracker calls, a
    total sleep budget of `EPIC_API_MAX_SLEEP_S` (900 s) per effect across all
    its retries; a reset beyond the remaining budget fails the effect, which
    then follows its retry rule. There is no thread kill.
  - `stop` and `replace` stop issuing calls at once and exit after the running
    call returns. The `replace` client waits at most 60 s for the lock; past
    that it prints `engine draining; the new engine starts when it exits`,
    exits 0, and the next `ensure` starts the new engine.
  - A closed effect channel turns every later call into
    `failed: effect thread stopped`; the engine keeps serving.
  - In process: launch, cleanup, checkpoint, prompt (`STATUS?`), refresh,
    close. `TOOLU_EPIC_SCRIPTS` affects only merge and merge-reconcile.
  - Known limit: one engine-wide effect thread means one epic's long tracker
    wait delays the others' effects; the state thread and status stay live.
- **Stages.** An issue occupies a slot when its stage is one of `starting`,
  `uncertain`, `replacing`, `running`, `awaiting_merge`, `cleaning`,
  `cleanup-incomplete` (TypeScript `OCCUPIED_STAGES`), or it has a pending
  action. The empty stage (never launched) is open: it can launch when ready.
  `merged` and `abandoned` are done. `closed` (a PR closed without merge,
  already a `pr-closed` judgment) and any unknown stage neither occupy a slot
  nor launch. This is a decision, not parity: TypeScript `classify` called
  such an issue `ready` when its tracker issue was open, and the orchestrator
  LLM decided. In the engine a `merged` issue whose tracker issue was seen
  closed after its merge (settle closed it, or `is_closed` reported it; the
  record gets `tracker_closed_at`) and is open again after a refresh raises
  one `reopened` judgment and is not relaunched (recovery.md already asks a
  human to read the reopening comment). `reopened_raised: true` in the record
  keeps it to one judgment across restarts. An `abandoned` issue keeps its
  tracker issue open by design (abandon never closes it) and raises nothing.
  `logic::busy`, `in_flight`, `launchable` and the snapshot overlay use these
  sets.
- **Epics.** `World.epics` holds per registered epic: state dir, tracker,
  reference, default repo, `max_parallel` (per epic; it replaces the global
  value), the pool from `pool.json`, refresh state (pending, failures, retry
  deadline), the current batch, `complete`, `closed`, and its epic-level
  pending action (refresh or close). Epic-level journal records carry the
  epic key in `key`.
- **Refill.** When an epic has free slots, the engine forms a batch at that
  moment: ready issues (no open blocker, not held, not deferred, not
  occupied, not done) in `pick_batch` order — chain length, then unblocked
  count, then repo load, repo, number (TypeScript parity; Jev 0.94) — cut to
  the free slots. It records the batch and arms one launch at a time from it.
- **Launch action.** Run the coupling check against the members of its batch
  ahead of it first, then route the issue (keep a valid saved route;
  otherwise score and pick a host), then launch in process. Checking before
  routing means a member that later waits on capacity was already compared,
  so a later batch cannot launch it unchecked against the members it was
  batched with. Outcomes:
  - `applied` → stage `running`;
  - `hold` → `hold` written to the record, judgment `coupling-hold` naming the
    partner, journal `hold`;
  - `wait` (no host capacity, shared capacity exhausted, GitHub budget under
    its floor) → no judgment, retried after one tick; `launch-wait` is
    journaled on the first wait of a streak and once more when the streak
    ends, with the number of waits in the note;
  - anything else → judgment `failed` with the reason; the stage is the
    record's (`uncertain` when the launch reserved ownership).
- **Relaunch counter.** `Issue.launches` counts relaunches after `gone` only.
  The first launch, a `wait` retry, a reroute and a failed launch do not
  change it. The launch record's own `launches` field (successful agent
  starts) is separate and is not overlaid onto it. `gone` with `launches < 2`
  increments it and relaunches; at 2 it raises `relaunch-limit`.
- **Recovery the LLM did:** `gone` → snapshot, relaunch with the resume prompt
  and captured session (unchanged rule above). Host limit (a rate-limited
  `failed` report or a provider-limit line) → cool the host for
  `EPIC_HOST_COOLDOWN_MIN` minutes (default 60, now read from the
  environment), no `failed` judgment, journal `reroute`, then route the issue
  with reroute: a free, non-cooling host gets it with replace semantics in the
  same worktree (resume prompt); none free → `launch-wait` until a slot or the
  cooldown ends. GitHub budget under its floor → launches and merges held
  (`github_budget`, which now reads `EPIC_GH_CORE_FLOOR` and
  `EPIC_GH_GRAPHQL_FLOOR`, defaults 1000 and 500 — the single owner of the
  floors). Stall → one `STATUS?` sent in process through herdr, then the
  `stall` judgment if the phase does not move (unchanged rule).
- **Merge bridge.** A deferred merge raises `merge-ready` (note: `run toolu
  epic gate --pr <repo>#<n> --merge`). A merge the PR detector sees moves a
  running issue to `cleaning` and arms cleanup (it no longer writes `merged`
  directly). `toolu epic gate --merge --key K` sends socket `event merged`
  so the engine reacts at once.
- **Cleanup action.** In-process finish (not `--abandon`). `merged` → settle
  the sub-issue (close and tick) unless `Tracker::is_closed` reports it closed,
  then mark the epic for refresh. `cleanup-incomplete` → judgment, lease kept.
- **Refresh.** Rebuilds `graph.json` for one epic and overlays new sub-issues,
  open blockers, classification, chain and unblocks. Triggered by #447's
  `EpicChanged`, by a finished cleanup, and every 15 minutes for Jira and
  Linear epics. A failed refresh keeps the previous graph, stays pending, and
  is retried every `watch::CHECK_MS` (180 s, fixed); the third consecutive
  failure raises one `refresh-failed` judgment (redacted error), and success
  clears the count.
- **Close.** When an epic's refreshed graph is complete and no issue is
  occupied, the engine posts the summary and closes the epic once, journals
  `epic-closed`, and raises `epic-complete`.
- **Holds.** `issues/<key>.json` gains `hold: {kind: "coupling", with, score,
  source, at}`. `toolu epic release <key>` clears it, sets
  `coupling_released: true` so the next launch skips the check, and journals
  `release`. Holds end only on release (Jev 0.74 for release-only over
  automatic release).
- **Judgment kinds added:** `merge-ready`, `epic-complete`, `refresh-failed`,
  `reopened` (`coupling-hold` already exists in `commit.rs`). Kinds are strings; no
  closed set exists to update.
- **Socket protocol** rises from 1 to 2 for the new ops and the changed effect
  semantics, so a newer client replaces an older engine through the existing
  `replace` handshake.

### Recovery per action

| Action | Intent without done after a restart |
|---|---|
| launch | read `issues/<key>.json`: `running`/`awaiting_merge` → done; `starting`/`uncertain`/`replacing` → judgment `failed` ("uncertain launch: inspect, then `toolu epic launch --reprompt` or `--replace`"), no agent start; no record or no stage → launch again |
| cleanup | run finish again (it is idempotent and refuses over unsafe state) |
| checkpoint, refresh | run again |
| close | `Tracker::epic` reports closed → done; open → judgment `failed` ("epic close uncertain"), never a second comment |
| merge | unchanged script `reconcile` |

`recover::action_of` learns `refresh` and `close`; `commit_reconcile` is
per action instead of assuming a merge.

### Coupling check

- Compared set: the members ahead of the candidate in its batch whose launch
  outcome was `applied` or `wait` (a waiting member still launches later, so
  a later member that overlaps it is held). Members that were held, skipped
  or failed are not compared. Because a batch launches in order, every member ahead has an
  outcome when the candidate is checked. `wait` is a final outcome for the
  batch: the member leaves it and becomes a fresh candidate of a later batch.
- Batch lifetime: a batch is formed when a refill finds free slots and is
  dropped only when every member has an outcome; the next refill forms a new
  one. While a batch is alive, refill forms no other batch for that epic;
  slots that free meanwhile wait for the next batch. Pause stops issuing its
  launches and resume continues the same batch. A refresh in the middle of a batch updates the graph
  (order, blockers, new issues) but neither drops the batch nor forgets its
  applied members; a member the refresh shows done or blocked gets the
  outcome `skipped` and is not launched. A batch of one
  (one free slot, the steady state) has nothing to compare and launches
  directly. The check therefore never compares against issues launched in
  earlier batches; that is the issue's wording ("over the batch's titles and
  bodies"), kept with Jev 0.74.
- With `TYPESAFE_API_KEY`: one Jev `noul` per pair ("Do `issues.a` and
  `issues.b` change the same module, schema or migration so that working them
  in parallel would force one to rebase over the other?") over titles, labels
  and excerpts; hold at P ≥ 0.80 (between Jev's 0.75/0.90 split).
- Without Jev: overlap when the two share a path token, a migration
  identifier, or a `module:`/`component:`/`path:` label. A path token matches
  `[A-Za-z0-9_.-]+(/[A-Za-z0-9_.-]+)+`, is not part of a URL, and either ends
  in a segment with a `.ext` or has two or more `/` (so `and/or`, `CI/CD`,
  `read/write` never match). A migration identifier matches
  `\d{3,}[_-][A-Za-z0-9_-]+` or `V\d+__[A-Za-z0-9_]+`.
- A Jev failure falls back to the heuristic and journals the reason.

### Native SessionStart hooks

The three Bun entries become three native entries with the same names, each
from `cargo xtask print-hook epic-orchestrator SessionStart <name>`
(`toolu epic-orchestrator hook <name> --event SessionStart --plugin-root …`,
which the fast path accepts because `epic-orchestrator` is a leaf plugin
owner). Each prints one output document, as its script did, so no envelope
merging is needed. `crates/cli/src/hook/plugins.rs` gains `epic_hook`
dispatching to `toolu_epic_orchestrator::hook::{check_binary, check_deps,
engine_ensure}`:

- `check-binary` — `native_toolu_advice` as `SessionStart` context. The
  session-id extraction and advice-to-output step move into one
  `toolu_runtime::startup::native_toolu_hook_output(env, stdin) -> Option<String>`
  (main's #512 added it first under that name), and `jev`'s and
  `toolu-review`'s `check_binary` call it too, so no copy
  exists for jscpd to find; their existing tests keep covering their output.
  Silent when native `toolu` is on the agent's `PATH`.
- `check-deps` — `codex_dependency_notice` for `toolu@toolu`,
  `delivery-flow@toolu`, `toolu-review@toolu`, `pr-babysit@toolu`,
  `brainstorm@toolu`, each style; silent on Claude Code and when all are
  installed and enabled.
- `engine-ensure` — `control::ensure` in process (no `toolu` subprocess);
  silent on success; on failure `{"systemMessage": "epic engine: <detail>"}`
  with the detail cut to 200 characters. `ensure` now reports a malformed
  `registry.json` instead of treating it as empty.

All three exit 0. `benchmarks/cases/hooks/payloads.json` keeps its three
`epic-orchestrator/*` keys.

### Markdown

`SKILL.md`, `commands/epic.md`, `references/worker-brief.md` and
`references/recovery.md` swap every script for its verb and replace the watcher
section with `toolu epic start` plus `toolu epic wait` and the judgment table
(including `merge-ready` → `toolu epic gate … --merge`, `coupling-hold` →
`toolu epic release`). Preflight checks `toolu --version` instead of `bun`.
The brief renders `toolu epic report <phase> --status-file <file>`,
`toolu epic job -- …` and, for Jira, `toolu epic jira issue get <KEY>`. The
template ships in the binary (`include_str!` of the plugin file).

## Interfaces / Schema

```text
toolu epic graph <EPIC> [--max N] [--tracker github|jira|linear] [--repo O/R] [--out FILE] [--save] [--dry-run]
toolu epic route --graph FILE [--hosts SPEC] [--issue REF]... [--no-jev] [--reroute] [--dry-run]
toolu epic launch --graph FILE --issue REF [--kind K] [--model M] [--effort E]
                  [--permission-mode MODE] [--safe] [--replace] [--force] [--reprompt] [--dry-run]
toolu epic finish <STATE_DIR> <KEY> [--abandon] [--dry-run]
toolu epic close --graph FILE [--dry-run]
toolu epic release <KEY> [--dry-run]
toolu epic gate --pr REF [--issue REF] [--epic REF] [--merge] [--auto] [--method M]
                [--state-dir DIR] [--key KEY] [--dry-run]
toolu epic jira issue get <KEY>
toolu epic probe --check-evidence [FILE]
toolu epic-orchestrator hook check-binary|check-deps|engine-ensure --event SessionStart --plugin-root DIR
```

- **Output.** `graph` and `route` print today's tables; with `--json` the
  `graph.json` document and `{routes, note}`. `launch --dry-run` prints `# <ref>`,
  the commands, `# brief -> <path>`, a blank line and the brief; with `--json`
  `{dry_run, issue, host: {kind, model, effort}, commands, agent_args,
  resume, prompt, brief_path, brief, exclude}` where `agent_args` is the
  argv after `--` in `agent start`, `resume` is whether it resumes a captured
  session, `prompt` is the start or resume prompt text, and `exclude` is the
  marker plus patterns OpenCode adds to `info/exclude` (empty for other
  hosts). A record with `launches > 0`, the same `kind` and a `session_id`
  dry-runs the resume path (`--session <id>` on OpenCode). Live `launch`,
  `finish`, `close`, `gate` and `release` print their JSON document with or
  without `--json` (as the scripts did). `close --dry-run` prints the table.
  `probe` prints the three summary lines; `jira issue get` the issue JSON.
- **`--dry-run`** never writes a file, lease, ref, journal line or tracker item
  and starts no process that writes. `launch --dry-run` makes no network call
  (default branch `main`, as today). `graph --dry-run` reads the tracker and
  writes nothing even with `--save`/`--out`. `jira` and `probe` are read-only
  and take no `--dry-run`.
- **Exit codes** (`toolu_protocol::exit::Exit`): 0 success; 1 failure (message
  on stderr; `finish` when the result is `cleanup-incomplete`; `close` when the
  epic is incomplete); 64 usage (clap, bad host spec, bad reference format).
  `launch`'s stderr on failure is `{"issue": REF, "error": TEXT,
  "native_exit_code": N|null}`, replacing the script's use of the native code
  as its own exit status.
- **`release <KEY>`**: the engine applies it over socket op `release` when
  live; otherwise the CLI finds `issues/<KEY>.json` in the registered epics
  (`registry.json`). Unknown key → exit 1 `no registered issue <KEY>`; a key
  with no hold → exit 1 `<KEY> is not held`.
- **Files unchanged in shape:** `graph.json`, `routes/<key>.json`, `pool.json`,
  `hosts.json`, `issues/<key>.json` (launch record plus `hold`,
  `coupling_released`, `last_gate`, `removal_intent`), `status/<key>.json`,
  `briefs/<key>.md`, `resources.json` leases (`pendingHost`, `worktree`,
  `pane`, `session`). New record fields in `issues/<key>.json`: `hold`,
  `coupling_released`, `tracker_closed_at`, `reopened_raised`.
- **Journal names added:** `refresh-intent`/`-done`, `close-intent`/`-done`,
  `launch-wait`, `reroute`, `hold`, `release`, `settle`, `epic-closed`,
  `refresh-error`, `coupling-fallback`.
- **Socket ops added (protocol 2):** `release {key}`, `event {key, event:
  "merged"}`.
- **Environment:** kept — `EPIC_STATE_HOME`, `EPIC_REPO_ROOTS`,
  `EPIC_ROUTING_FILE`, `EPIC_API_ATTEMPTS`, `EPIC_API_BACKOFF_MS`,
  `EPIC_API_MAX_SLEEP_S`, `EPIC_GH_CORE_FLOOR`, `EPIC_GH_GRAPHQL_FLOOR`,
  `EPIC_HOST_COOLDOWN_MIN` (now honored by the engine), the Jira, Linear and
  Jev variables. Removed — `EPIC_JEV`: Jev runs in process, so there is no
  `jev.sh` path to override; `TYPESAFE_API_KEY` alone enables it. Removed with
  the watcher, whose behavior #434's engine already replaced with fixed rules:
  `EPIC_STALL_REPEAT_MIN` (the engine sends one `STATUS?` then raises one
  `stall` judgment; `toolu epic ack` restarts the clock), `EPIC_LIMIT_SCAN_MIN`
  (the engine scans a pane tail once per phase), `EPIC_HERDR_BACKOFF_S` and
  `EPIC_HERDR_BACKOFF_MAX_S` (30 s doubling to 5 min, fixed), and
  `EPIC_CHECKPOINT_BATCH` (one worktree per checkpoint step, every 15 min).
  The README stops documenting them.
- **Shared fixtures:** `fixtures/epic/hosts.json` — `{"version": 1, "cases":
  [...]}` with `kind: "args"` (`host`, `opts`, `argv`: the 24 combinations the
  pr-babysit parity test checks plus resume and safe cases), `kind: "alias"`
  (`name`, `host`), `kind: "limit"` (`text`, `matches`) and one `kind:
  "table"` (`hosts`: the default routing table). Registered in
  `fixtures/index.json` as suite `epic-hosts`. `fixtures/epic/epic248-graph.json`,
  `pr290-rollup.json`, `epic248-body.md` and `epic203-body.md` move from
  `scripts/fixtures/`. `fixtures/epic/herdr-calls.json` records the herdr
  0.9.3 (protocol 22) method names and param keys every `herdr::Call` must
  match.
- **Seam:** `tools/toolu-conformance/src/harness/entry-command.ts` gains
  Rust-only CLI entries `epic-orchestrator/launch` → `toolu epic launch` and
  `epic-orchestrator/report` → `toolu epic report`, resolved to the built
  binary (`TOOLU_RUST_BIN_DIR`, else `target/debug`, else `target/release`)
  whatever `TOOLU_IMPL` says, because no Bun bundle remains.
  `fixtures/rust-ported.json` and `benchmarks/hook-budgets.json` are
  unchanged (leaf-plugin hooks such as `statusline/session-start` are not
  listed there either); `benchmarks/cases/hooks/payloads.json` keeps its three
  `epic-orchestrator/*` keys, now measured natively.

## Failure modes and edge cases

- **Tracker errors** follow the retry mapping; mutations never retry a
  transient; a reset beyond the cap fails with its time. In the engine a
  refresh failure keeps the old graph and retries on the fixed 180 s deadline,
  with one `refresh-failed` judgment at the third failure.
- **Bad references:** unknown epic or issue reference, ambiguous bare
  Jira/Linear key, bad `JIRA_API_VERSION` or `JIRA_BASE_URL`, missing
  credentials — refused before any request with the TypeScript message (exit
  64 for a malformed reference, 1 for missing configuration).
- **Missing credentials in the engine:** refresh, settle and close fail with
  the redacted message naming the variable; refresh raises `refresh-failed`
  after three tries; settle and close raise `failed`.
- **Cycle:** no waves, no batch, `dependency-cycle` judgment (unchanged).
- **Route:** a persisted `host: null` refuses even `--force` before any
  mutation; malformed routes, pools and records refuse before ownership;
  duplicate, nonpositive, fractional or non-numeric caps refuse.
- **Launch:** shared capacity is checked before any workspace exists; a native
  failure after reservation leaves `uncertain` with `lifecycle_error`,
  `native_error` and the lease, and a retry needs `--reprompt` or `--replace`;
  `cleaning`/`cleanup-incomplete` refuse launch; a host change needs
  `--replace`; OpenCode without a 1.x `opencode` stops before side effects; a
  listed worktree without a path is refused.
- **Replacement:** destination reserved before the source stops; refused while
  old workload or managed jobs remain; both slots stay occupied until the
  source exit is verified.
- **Finish:** refuses when the PR is not merged (non-abandon), when the record
  is missing, malformed or partial, when owned workload or a managed job lives,
  when dirty work could not be snapshotted, when a removed worktree is still
  registered; every refusal leaves `cleanup-incomplete` and the lease.
- **Gate:** merge pinned to the verified head; a moved head fails the merge;
  `--admin` only after a protection-only refusal; an uncertain merge (timeout)
  is reported `uncertain` and never retried.
- **Engine crash:** per the recovery table; no second agent, no second epic
  comment.
- **Concurrency:** one effect at a time; the per-issue launch lock
  (`launch-locks/<key>`) is shared by the CLI verbs and the engine, so a manual
  launch or finish and the engine never overlap on one issue.
- **Protocol skew:** a protocol-1 engine is replaced by the protocol-2 client's
  handshake; a report sent meanwhile is spooled.

## Acceptance criteria

- **AC-1:** Given GitHub, Jira (v2 and v3, on the loopback HTTPS fixture) and
  Linear fixtures, the trackers return the TypeScript-normalized epic and
  children (refs, state, repo, blockers, PRs, labels, excerpt) and close and
  tick as today; a 5xx on a write is not retried, a 5xx on a read is, a 429
  waits for its `Retry-After`, a reset beyond the cap fails with its time.
- **AC-2:** Given the epic #248 graph, `toolu epic graph` reproduces the
  classification, waves, unblocks, chain, launch batch, cycle and missing
  checkouts; `--save`/`--out` write `graph.json`; `--dry-run` writes nothing.
- **AC-3:** `toolu epic route` gives the TypeScript tiers, hosts, models,
  efforts and persisted routes, with Jev in process and the heuristic without
  it; every occupied stage and every shared or reserved lease consumes
  capacity; cooling hosts are skipped.
- **AC-4:** `hosts` reproduces every case of `fixtures/epic/hosts.json`, and
  the pr-babysit parity test passes against the same file.
- **AC-5:** `toolu epic launch --dry-run` on the #248 graph prints the exact
  clone/fetch/worktree/agent/prompt commands and the brief for Claude, Codex,
  Cursor (safe), OpenCode and a missing checkout, and leaves the state
  directory, resource root and checkout byte-identical.
- **AC-6:** Live `toolu epic launch` refuses before any ownership on a null
  route, a malformed route, pool or record, an incomplete stage or a host
  change without `--replace`; refuses at capacity before external work; after
  a real missing dependency leaves `uncertain` ownership and refuses a blind
  retry; stops before side effects without a 1.x `opencode`; refuses a listed
  worktree without a path; the OpenCode exclude is appended once.
- **AC-7:** Session capture attests exactly one recorded Claude, Codex, Cursor
  or OpenCode session and never guesses.
- **AC-8:** With a live process group whose cwd is inside the worktree,
  `toolu epic finish` refuses and leaves `cleanup-incomplete`; a `--replace`
  launch with old workload alive refuses; the Linux and macOS inventories find
  descendants and exclude the caller and the shell.
- **AC-9:** `toolu epic finish` reproduces the eleven TypeScript finish
  outcomes (abandon snapshot, clean pushed, PR-less refusal, malformed
  records, usage, stages, legacy fence adoption, branch retry, crash after
  prepared removal, absent worktree, incomplete terminal).
- **AC-10:** `checkpoint::snapshot` reproduces the ten TypeScript snapshot
  outcomes (nothing at risk, tracked/staged/untracked, reflog dedup, unpushed
  pinning, retained index, HEAD reset, corrupt index, locked index, missing
  index, serialized concurrency).
- **AC-11:** `toolu epic close --dry-run` prints the summary table with admin
  merges noted; a complete epic closes in its tracker; an incomplete epic is
  refused with its open refs.
- **AC-12:** `toolu epic gate` reproduces the merge-gate cases (check buckets,
  checklist ticks, protection detection, auto-merge pinning and arm/disarm,
  recorded auto-merge state) and, on the GitHub fixture, returns `merge`,
  `rebase`, `fix`, `wait`, `merged` and `closed` for the matching PR states,
  merges pinned to the head and retries with admin only after a
  protection-only refusal.
- **AC-13:** `toolu epic jira issue get PAY-12` prints the issue JSON from the
  fixture and refuses bad arguments or missing credentials before HTTP.
- **AC-14:** `toolu epic probe --check-evidence` validates
  `docs/toolu/evidence/epic-hosts-v1.json`, prints the summary with every gap,
  and rejects a record missing a host.
- **AC-15:** A scripted epic of four issues where D is blocked by C (A and B
  independent), on the engine with scripted workers, the merge script seam,
  the herdr socket fixture and the GitHub fixture: A, B and C launch first, D
  launches only after C merges, every issue is finished, and the epic is
  closed in the tracker with no `wait` caller.
- **AC-16:** An issue whose agent disappears is snapshotted and relaunched
  twice and the third disappearance raises `relaunch-limit`; a reroute before
  the disappearances does not change the count.
- **AC-17:** With Claude cooling down, a ready issue routes to a free Codex
  slot; a worker that reports `failed` with `rate-limited` cools Claude for
  `EPIC_HOST_COOLDOWN_MIN` minutes and is moved to Codex in the same worktree
  (replace, resume prompt), with no `failed` judgment.
- **AC-18:** Two batch issues that name the same migration (Jev fixture at
  P=0.93, and the heuristic) hold the lower-priority one with
  `coupling-hold`, and the journal has a `hold` line; two unrelated issues
  launch together; "and/or" or a URL in both bodies does not hold; a lone
  candidate refilling a single slot launches without a check even when it
  names a running issue's migration; a member that was held is not compared
  with later members; a refresh between two launches of one batch keeps the
  first member's applied outcome, so the second is still compared, and so
  does a pause and resume between them; in batch [A, B] where B names A's
  migration and no host is free when B is reached, B is held (the check
  precedes routing), and where B does not overlap it waits and launches
  later; in batch [A, B, C] where B waits and C names B's migration, C is
  held;
  `toolu epic release <key>` journals `release` and the held issue launches;
  `release` of an unknown key fails with `no registered issue <KEY>` and of an
  unheld key with `<KEY> is not held`, both exit 1, with and without a live
  engine.
- **AC-19:** With one free slot and three ready issues the engine launches by
  chain length, then unblocked count: given X (chain 3, unblocks 2) and Y
  (chain 2, unblocks 4), X launches first. This keeps `pickBatch` parity
  (Jev 0.94); the PR body names the issue scenario ("unblocks the most work")
  and this reading of it.
- **AC-20:** A Jira epic `PAY-7` with two children, one blocked by the other
  through an "is blocked by" link, launches only the unblocked child.
- **AC-21:** `toolu epic-orchestrator hook check-deps` prints the Codex
  dependency notice naming each missing or disabled plugin and its install
  command, and is silent on Claude Code and when all are installed and
  enabled; `hook check-binary` prints the native-binary advice once per
  session id and nothing when native `toolu` is on `PATH`; `hook
  engine-ensure` is silent on success and reports a malformed `registry.json`
  as `epic engine: …` in a `systemMessage`; all three exit 0.
- **AC-22:** `plugins/epic-orchestrator/scripts`, `hooks/src` and `hooks/dist`
  are gone; no skill, command, agent or reference names `scripts/`,
  `hooks/dist/`, a Bun bundle or `bun`; `cargo xtask check-markdown-cli`
  passes; the brief has no placeholder left for any host and tracker.
- **AC-23:** The OpenCode live epic-worker test takes the fresh and resume
  `agent_args`, the `prompt`, the `brief` and the `exclude` lines from
  `toolu epic launch --dry-run --json` and reports through `toolu epic
  report`, both resolved by the seam's Rust-only entries, and checks the
  pinned host's version itself; the pr-babysit hosts parity test reads
  `fixtures/epic/hosts.json`; neither imports `plugins/epic-orchestrator`.
  The dry-run JSON contract it relies on is covered locally by
  `crates/cli/tests/epic_launch.rs` (OpenCode fresh and resume cases); the
  live test runs in the `opencode` CI job, or locally with
  `TOOLU_LIVE_OPENCODE=1` and a cached pinned host.
- **AC-24:** A deferred merge raises `merge-ready`; a merge seen by the PR
  detector or sent as `event merged` arms cleanup, which finishes the issue and
  refreshes the epic.
- **AC-25:** While the effect thread is blocked on a barrier, the engine
  answers `status` and records a report; releasing the barrier commits the
  effect.
- **AC-26:** After a crash between intent and done, a restart starts no second
  agent for a `starting`/`uncertain` launch, reruns cleanup, and does not
  re-comment an epic close.
- **AC-27:** `EpicChanged` refreshes a GitHub epic and a Jira epic refreshes
  after 15 minutes; a refresh that fails once is retried 180 s later and
  succeeds, and three failures raise one `refresh-failed`.
- **AC-28:** With the GitHub budget under its floor the engine starts no
  launch and journals `launch-wait`; a stalled worker gets one `STATUS?`
  through the herdr fixture, then a `stall` judgment.
- **AC-29:** A launch counts every occupied stage: with `max_parallel` 2, one
  `uncertain` and one `cleanup-incomplete` issue, a ready issue does not
  launch; a `merged` issue whose tracker issue was closed and reopens raises
  one `reopened` judgment (one across a restart) and is not relaunched; an
  `abandoned` issue with its tracker issue open raises nothing and does not
  launch; a `closed` issue does not launch.
- **AC-30:** No sentinel `GH_TOKEN`, `JIRA_PAT`, `JIRA_API_TOKEN`,
  `LINEAR_API_KEY` or `TYPESAFE_API_KEY` appears in the journal, attention,
  status or error output after the tracker failure paths.
- **AC-32:** While the effect thread runs a call, a `STATUS?` prompt the stall
  rule produces parks, and the `Journal` and `Status` steps queued after it are
  written at once; the parked prompt is sent when the call returns.
- **AC-33:** A launch that cannot get a slot for five ticks journals
  `launch-wait` once at the first tick and once when it launches, the second
  line carrying the count 5.
- **AC-34:** An engine effect whose tracker answers three 429s with
  `Retry-After` values summing beyond `EPIC_API_MAX_SLEEP_S` fails once the
  budget is spent, without sleeping past it, and its retry rule applies.
- **AC-35:** `toolu epic engine --replace` while an effect is running stops new
  calls at once; when the effect outlasts 60 s the client prints `engine
  draining; the new engine starts when it exits` and exits 0, and the old
  engine exits after the effect returns.
- **AC-36:** A protocol-2 client meeting a protocol-1 engine spools its report
  and sends `replace` (the existing handshake), and the report is ingested by
  the new engine.
AC-31 is retired (relabelled); AC-37 is the final gate.

- **AC-37:** `cargo xtask gate` passes with no exemption; `docs/cli/`, the
  commands snapshot and the generated OpenCode surfaces are current; the
  Linux idle-engine RSS test stays within its limit; the stripped release
  binary size before and after is recorded in the PR.

## Acceptance evidence

| AC | Real input or fixture | Expected result and boundary | Check |
|---|---|---|---|
| 1 | HTTPS fixture replies copied from the TypeScript tracker, Jira, Linear and ratelimit tests | normalized structs; write 5xx single request; 429 waits | `cargo test -p toolu-epic-orchestrator tracker retry` |
| 2 | `fixtures/epic/epic248-graph.json`, real repos with `origin` URLs | levels/batch/cycle; `--dry-run` leaves no file | `cargo test -p toolu-epic-orchestrator graph`, `cargo test -p toolu-cli --test epic_graph` |
| 3 | temp state and resource roots, Jev HTTPS fixture | same routes; null host when full | `cargo test -p toolu-epic-orchestrator route` |
| 4 | `fixtures/epic/hosts.json` | exact argv | `cargo test -p toolu-epic-orchestrator hosts` (the TypeScript parity test left with #512) |
| 5 | #248 graph in a temp state dir | stdout lines; byte-identical trees | `cargo test -p toolu-cli --test epic_launch` |
| 6 | malformed files, `maxAgents: 1`, `PATH=""`, stages | refusal texts; no lease; `uncertain` record | `cargo test -p toolu-cli --test epic_launch` |
| 7 | temp Claude/Codex/Cursor homes, `opencode` on `PATH` printing a session list | exact id or none | `cargo test -p toolu-epic-orchestrator session` |
| 8 | real `sleep` child with cwd in a temp worktree; herdr fixture | refusals | `cargo test -p toolu-epic-orchestrator workload finish admission` |
| 9 | temp checkout, worktree and records per case | records and exit codes | `cargo test -p toolu-cli --test epic_finish` |
| 10 | real git repos per case, `GIT_TRACE2_EVENT` | snapshot fields, reflog and trace counts | `cargo test -p toolu-epic-orchestrator checkpoint` |
| 11 | graph and records; GitHub fixture | table; requests | `cargo test -p toolu-epic-orchestrator close` |
| 12 | rollups and bodies; GitHub fixture PR states | verdicts; merge `sha`; admin only after protection text | `cargo test -p toolu-epic-orchestrator gate` |
| 13 | Jira fixture issue | JSON; usage before HTTP | `cargo test -p toolu-epic-orchestrator jira_cli` |
| 14 | committed evidence; copy without `opencode` | summary; exit 1 | `cargo test -p toolu-cli --test epic_probe` |
| 15 | four-issue GitHub fixture, herdr socket fixture with real git worktrees, scripted workers, merge script | journal order; epic close requests | `cargo test -p toolu-epic-orchestrator scenario_chain` |
| 16 | herdr fixture losing the pane three times after a reroute | 2 relaunches, then `relaunch-limit` | `cargo test -p toolu-epic-orchestrator scenario_gone` |
| 17 | resource cooldown for claude; rate-limited report | `codex` route; replace on the same worktree | `cargo test -p toolu-epic-orchestrator scenario_cooldown` |
| 18 | issues naming `0042_add_users`; unrelated pair; "and/or" and URL pair; Jev fixture P=0.93; a one-slot refill with a lone candidate naming a running issue's migration; a three-member batch whose first member is held; a refresh, and separately a pause and resume, between two launches of one batch; batch [A, B] with no free host at B, overlapping and not; batch [A, B, C] with B waiting and C overlapping B; `release` of an unknown and of an unheld key with a live engine and with none | hold with a `hold` journal line; lone candidate launches; held member not compared; second member still compared after refresh and after pause/resume; B held before routing when overlapping, launched later when not; C held against waiting B; `release` journals and launches; error texts and exit 1 | `cargo test -p toolu-epic-orchestrator --lib coupling`, `--lib scenario_coupling`, `--lib holds`, `--lib control` |
| 19 | X chain 3/unblocks 2, Y chain 2/unblocks 4, Z chain 1 | X first | `cargo test -p toolu-epic-orchestrator refill` |
| 20 | Jira fixture `PAY-7` | blocked child not launched | `cargo test -p toolu-epic-orchestrator scenario_jira` |
| 21 | Codex home with a `codex` executable printing a plugin list; a session id; a `PATH` with and without native `toolu`; malformed registry | notice; advice once; `systemMessage`; exit 0 | `cargo test -p toolu-cli --test epic_hook` |
| 22 | repository tree | paths absent; drift gate; filled brief | `cargo xtask check-markdown-cli`, `cargo test -p toolu-epic-orchestrator brief` |
| 23 | seam entries, hosts fixture | no epic plugin import | `bun test tools/toolu-conformance/src/harness/__tests__`; `bun run test:opencode` in CI |
| 24 | ready issue without scripts; PR fixture flips to merged | `merge-ready`; cleanup intent | `cargo test -p toolu-epic-orchestrator merge_bridge` |
| 25 | `Executor::Thread` with a barrier effect | status reply and report while blocked | `cargo test -p toolu-epic-orchestrator effect_thread` |
| 26 | journals with intents only; records per stage | no `agent.start`; one comment | `cargo test -p toolu-epic-orchestrator recover` |
| 27 | GitHub `EpicChanged`; Jira epic, clock +15 min; fixture failing once then three times | refresh lines; one judgment | `cargo test -p toolu-epic-orchestrator refresh` |
| 28 | budget under floor; stalled phase | `launch-wait`; `STATUS?` then `stall` | `cargo test -p toolu-epic-orchestrator refill schedule` |
| 29 | records `uncertain` and `cleanup-incomplete` with `max_parallel` 2; a `merged` record with `tracker_closed_at` whose tracker issue reopens, then an engine restart; an `abandoned` record with an open tracker issue; a `closed` record | no launch; one `reopened` before and none after the restart; nothing for the abandoned one; no launch for `closed` | `cargo test -p toolu-epic-orchestrator --lib refill`, `--lib refresh`, `--lib recover` |
| 30 | sentinel credentials, failing fixtures | sentinel absent | `cargo test -p toolu-epic-orchestrator leak` |
| 32 | `Executor::Thread` blocked on a barrier; a stalled issue | journal and status written while parked; prompt after release | `cargo test -p toolu-epic-orchestrator --lib effect_thread` |
| 33 | engine with no free host for five ticks, then a slot | two `launch-wait` lines, count 5 | `cargo test -p toolu-epic-orchestrator --lib refill` |
| 34 | GitHub fixture answering 429 `Retry-After: 2` three times with `EPIC_API_MAX_SLEEP_S=3` | two requests, failure after about 2 s (the second wait exceeds the 1 s left); refresh retry deadline set | `cargo test -p toolu-epic-orchestrator --lib retry` |
| 35 | a live engine whose effect blocks on a barrier past the client wait; `control::replace_engine(paths, wait: Duration)` takes the limit as a parameter, the CLI passes 60 s and the test 200 ms | client message, exit 0; old engine exits after release | `cargo test -p toolu-epic-orchestrator --lib control` |
| 36 | a protocol-1 socket stand-in | spool file and `replace`; ingested later | `cargo test -p toolu-epic-orchestrator --lib client` |
| 37 | the branch | gate green; sizes in PR | `cargo xtask gate`; `cargo build --release -p toolu-cli` and `ls -l` before/after |

### TypeScript test inventory

Every deleted TypeScript case maps to a Rust test of the same meaning (named in
snake case) or to an existing test, or is listed as obsolete with the reason.

| TypeScript file | Destination |
|---|---|
| `admission`, `launch-issue`, `launch-state`, `launch-worktree`, `worktree-exclude`, `common` | `launch/*` unit tests and `crates/cli/tests/epic_launch.rs` |
| `checkouts`, `epic-graph` | `graph/*` and `refs` unit tests, `epic_graph.rs` |
| `checkpoint` | `checkpoint` unit tests; `snapshotActive` → `checkpoint::snapshot_active` (engine checkpoint wave) |
| `epic-close` | `close` unit tests |
| `finish-issue` | `finish/*` unit tests, `epic_finish.rs` |
| `hosts` | `hosts` unit tests over `fixtures/epic/hosts.json` |
| `jira-issue`, `jira-tracker`, `linear-tracker`, `trackers`, `ratelimit` | `jira_cli`, `tracker/*`, `retry` unit tests (`mapLimit` → `tracker::github` bounded fetch test) |
| `merge-gate` | `gate/*` unit tests |
| `probe-evidence` | `epic_probe.rs` |
| `route` | `route/*` unit tests; the two `jevCommand` cases and the `jev.sh` lookup case are obsolete (Jev in process, `EPIC_JEV` removed) |
| `session`, `workload` | `launch::session`, `workload/*` |
| `skill-contract` | `epic_markdown.rs` (skill and command name only `toolu epic`; preflight names `toolu`; brief filled) and `check-markdown-cli` |
| `report` | `crates/cli/tests/epic_report.rs` (six cases; usage exits 64) |
| `epic-watch` | case by case below |
| `job-waiting` | existing `waiting_jobs_test` |
| `runtime-packaging` | obsolete: no bundled Bun runtime remains |
| `check-deps`, `engine-ensure` | `epic_hook.rs` (all four `check-deps` cases; `engine-ensure` success and failure); the relative-`TOOLU_BIN` and no-native-`toolu` cases are obsolete (the hook runs inside native `toolu`) |

`epic-watch.test.ts`, case by case (E = existing test, N = new test):

| Case | Rust test |
|---|---|
| ready is reported once | E `report_test::report_ready_merges`; N `logic_test::a_repeated_ready_arms_one_merge` |
| blocked once per episode | E `schedule_test::events_record_blocked_gone_and_host_limits`; N `limit_test::blocked_is_raised_once_until_working` |
| gone only while running and herdr reachable | E `source_test::herdr_snapshot_gone`, `source_test::herdr_outage` |
| idle agent with old status is stalled but babysit is patient | E `schedule_test::babysit_uses_its_own_stall_limit` |
| stale working and idle agents repeat on a bounded persisted deadline | obsolete: #434's engine nudges once and raises one `stall` judgment, no repeat (`EPIC_STALL_REPEAT_MIN` removed); E `schedule_test::a_stall_nudges_once_then_becomes_a_judgment` |
| stall acknowledgement persists and defers the next bounded escalation | E `schedule_test::ack_clears_a_stall_nudge`; persistence across restarts is obsolete with the repeat |
| dependency outages preserve agent episodes and stall acknowledgement | E `source_test::herdr_outage`; N `source_test::an_outage_keeps_nudge_and_ack_state` |
| parked ready and needs-human workers do not raise stall events | E `schedule_test::quiet_phases_do_not_stall` |
| ageMinutes respects explicit timezone offsets … | obsolete: the engine keeps epoch milliseconds and parses no stamps |
| watch timing arguments reject nonfinite, negative, and zero pacing | obsolete: no watcher CLI flags |
| watch CLI rejects invalid timing before acquiring ownership … | obsolete: no watcher CLI flags |
| watch CLI acknowledges a persisted stall without polling herdr | E `schedule_test::ack_clears_a_stall_nudge`, `query_test::status_cli_round_trip_on_a_local_engine`; N `query_test::ack_round_trips_on_a_local_engine` |
| watch runtime reloads persisted checkpoint, budget, and dependency deadlines | E `watcher_test::watcher_engine` (herdr backoff reload); N `snapshot_test::watch_file_reloads_checkpoint_hold_and_budgets` |
| watch lock is exclusive and a stale release token cannot remove a new owner | E `watcher_test::watcher_engine` (`engine-busy`), `lock_test::a_live_lock_is_busy_and_a_dead_pid_is_reclaimed` |
| limitLine finds the newest limit line in a pane tail | E `limit_test::limit_line` |
| coolHost writes a cooldown that routing honors | N `route::pool` test `a_cooldown_written_by_limit_moves_the_route` |
| coolHost can publish the same cooldown to shared machine state | N `limit_test::a_cooldown_reaches_shared_state_for_the_configured_minutes` |
| a worker's rate-limited report cools its host and raises host-limited once | E `source_test::herdr_limit`; N `limit_test::a_rate_limited_report_cools_once`, `scenario_cooldown` |
| a record naming an unknown host is still reported, without a cooldown or a crash | N `limit_test::an_unknown_host_is_reported_without_a_cooldown` |
| an OpenCode worker's failure and disappearance reach the orchestrator | N `scenario_opencode` |

`epic-watch.ts`, `report.ts`, `job.ts` and `watch/*` are deleted without a new
port: #434 and #446 replaced them with the engine, `toolu epic report` and
`toolu epic job`.

## Documentation impact

- `plugins/epic-orchestrator/README.md`: install without Bun, verbs table,
  SessionStart hook, holds, merge bridge, Jira read command, environment
  (`EPIC_JEV` removed).
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`,
  `references/worker-brief.md`, `references/recovery.md`, `commands/epic.md`.
- `docs/cli/` (regenerated) and `crates/cli/tests/fixtures/commands_json.snap`.
- `docs/testing.md` (epic suites in Rust, seam entries), `docs/epic-orchestrator/README.md`,
  `docs/install.md` and `docs/gate-coverage-matrix.md` where they name scripts
  or Bun hooks; `fixtures/gate-coverage/inventory.json`; `fixtures/README.md`
  and `fixtures/index.json` (new `epic` files).
- Stale paths to fix: `tsconfig.json`, `knip.json`, `plugins/.oxlintrc.json`,
  `crates/xtask/src/pack.rs`, `tooling/src/pack-inventory.ts`,
  `tooling/src/__tests__/bundle-plugins.test.ts`,
  `tooling/src/__tests__/check-retired-plugin-references.test.ts`,
  `crates/epic-orchestrator/src/tests/snapshot_test.rs`,
  `crates/xtask/src/tests/bun_launcher_test.rs`,
  `crates/xtask/src/markdown_cli/tests/surfaces_test.rs`, `docs/runtime.md`.
- OpenCode surface tests that pin the old skill text:
  `tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts` and
  `generate-surface.test.ts` (they assert `epic-watch.ts`, `launch-issue.ts`
  and `S="${ROOT}/scripts"`); `opencode-port.ts` stops rewriting those lines.
  On OpenCode the watcher rule becomes a bounded loop of
  `toolu epic wait --max-seconds 480` (no background shell), in the skill and
  in `docs/opencode.md`, which `check-opencode-docs` regenerates.
- `.toolu/skills/verify-epic-watcher/SKILL.md` runs the engine commands
  (`toolu epic start`, `status`, `wait`) instead of `epic-watch.ts`;
  `tooling/src/__tests__/pack-closure.test.ts` stops naming the plugin's
  script files.
- OpenCode generator and scenarios that read `scripts/`:
  `tools/toolu-opencode/scripts/bundle-plugins.ts` (skips a plugin without
  `scripts/`), `tools/toolu-opencode/scripts/lib/opencode-port.ts`,
  `tooling/src/opencode-host/scenarios-package.ts` and
  `scenarios-paths.ts`, whose `$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/scripts/report.ts`
  probes become `toolu epic report` probes (the `opencode` job puts the built
  `toolu` on `PATH`).
- `AGENTS.md`: a key-file row for `crates/epic-orchestrator` actions and the
  `toolu_engine::resources` transitions.
- Generated OpenCode surfaces under `tools/toolu-opencode/generated/`.

## Open Questions

None blocking. Decided: the gate is a manual verb in this issue (scope
transfer above, Jev 0.80); `check-deps` and `probe` are ported (Jev 0.56,
0.62); chain-first refill (Jev 0.94); coupling over the batch with release-only
holds (Jev 0.74) at P ≥ 0.80; engine effects use the CLI retry policy (Jev
0.95); `EPIC_JEV` removed; `EPIC_HOST_COOLDOWN_MIN` honored.
