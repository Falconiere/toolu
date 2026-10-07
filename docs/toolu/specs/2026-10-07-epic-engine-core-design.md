# Epic engine core — Design

**Date:** 2026-10-07   **Status:** Approved   **Author:** Falconiere   **Topic:** Resident `toolu epic` engine, state machine and journal (#434)

## Problem

The orchestrator LLM is the control loop. A worker that reports `ready` waits
until that session runs the merge, and a dead session advances nothing. #434
replaces that loop with one resident process that applies the deterministic
transitions itself and returns a judgment event only when a person or a later
policy has to decide.

## Non-Goals

1. No second port of leases, policy, pressure or `run_managed_job` (#421 owns them).
2. No GitHub poller (#447), herdr subscription client (#446), graph/route/launch/finish (#435), or merge queue (#448).
3. No HTTP status server, and no edits to hub doctor/config/status/setup (#445) or pre-tool gates (#419).
4. No push to the base branch and no merge that did not go through the gate. The sandbox merge script is the stand-in for the gate until #448.

## Architecture

One process per `resource_home` (`TOOLU_RESOURCE_HOME`, else
`~/.local/state/toolu/resources`). A pid lock at `<root>/engine.lock` is taken
with exclusive create and released on exit; a dead pid is reclaimed. The
process is foreground by default. `toolu epic start <state-dir>` registers the
epic and detaches the engine through `toolu_runtime::process::detach`.

Threads: one accepts the Unix socket, one sleeps 30 seconds and sends a tick,
one applies the state machine. Channels connect them. The tick is
`toolu_epic_orchestrator::TICK` (30 seconds). Checkpoint cadence is 15 minutes.
Herdr probe backoff is 30 seconds, doubling to 5 minutes, stored in
`<root>/watch.json` with the TypeScript `WatchRuntime` fields.

Effects implement merge, cleanup, launch, reconcile, prompt and checkpoint.
`TOOLU_EPIC_SCRIPTS/<name>` is executed when that directory is set; otherwise
the action is journaled `deferred` and not executed. A script prints one JSON
object: `{"outcome":"applied"|"already"|"failed"|"unknown"|"hold"|"deferred"}`.
Reconcile prints `{"state":"merged"|"open"|"unknown"}`.

Crash recovery reads snapshots and the journal tail (current day and previous
day). An open `merge-intent` without `merge-done` is reconciled. `merged` skips
the call and continues to cleanup. `open` allows one call. `unknown` raises
`failed` and does not call. The same rule applies to cleanup and launch.

## Interfaces / Schema

Socket `<root>/engine.sock`, mode 0600. First server line:
`{"op":"hello","protocol":1}`. One JSON object per line, at most 64 KiB.
Protocol constant `PROTOCOL` is 1. `replace` is accepted at any protocol.
Other ops require a matching `protocol`.

Ops: `status`, `pause`, `resume`, `ack`, `answer`, `wait`, `report`, `event`,
`replace`, `stop`.

Judgment documents, and only these, satisfy `wait`:
`needs-human`, `failed`, `blocked`, `stall`, `dependency-cycle`,
`coupling-hold`, `cleanup-incomplete`, `relaunch-limit`.
Each is `{"kind","key","epic","note","seq"}`. At the wait limit the body is
`{"state":"waiting"}`.

`wait --max-seconds` defaults: Claude 2700; Codex, OpenCode, Cursor, Hermes 480.

Journal line (`<root>/journal/YYYY-MM-DD.jsonl`), closed keys
`seq`, `at`, `kind`, `name`, `epic`, `key`, `token`, `note`:
`kind` is `transition`, `action`, `prompt`, `judgment`, `admission` or `recovery`.
`note` is at most 1024 characters. A note containing `ghp_`, `github_pat_` or
`TYPESAFE_API_KEY` is stored as `[redacted]`. Files older than 90 days are
deleted on a tick. Appends take `<root>/journal.lock`.

Snapshots: `<epic>/issues/<key>.json`, `<epic>/status/<key>.json`,
`<epic>/graph.json`. Existing keys are preserved. Status updates set `phase`,
`pr`, `note`, `updated_at` (`YYYY-MM-DDTHH:MM:SSZ`) and append `history`.
Issue `stage` uses the TypeScript set `starting`, `running`, `cleaning`,
`merged`, `abandoned`, `cleanup-incomplete`.

Registry: `<root>/registry.json` `{"version":1,"epics":[{"key","state_dir"}]}`.
Pause: `<root>/pause.json` `{"all":bool,"epics":[string]}`.
Spool: `<root>/spool/<id>.json`, ingested in name order on start, skipped when
the journal already has `name=spool` for that `token`.

CLI:

- `toolu epic engine [--replace] [--ensure]`
- `toolu epic start <state-dir>`
- `toolu epic status [epic]`
- `toolu epic pause [epic]` / `resume [epic]`
- `toolu epic ack <key>`
- `toolu epic answer <key> <text>`
- `toolu epic wait [--max-seconds N]`
- `toolu epic report <phase> --status-file <file> [--pr N] [--note TEXT]`
- `toolu epic job <argv...>` (also `job -- <argv...>`)
- `toolu epic service install`
- `toolu epic planned` lists `graph route launch finish close release jira probe gate queue` for #435 and #448
- `toolu epic token new` unchanged

`report`, `wait` and `status` call `engine --ensure` when the registry is
non-empty and the lock is free. SessionStart hook `engine-ensure` does the same.

`event` values `gone`, `blocked`, `host-limited` are the source plug. `gone`
checkpoints and relaunches while `launches` is below 2, then raises
`relaunch-limit`. `blocked` raises `blocked`. `host-limited` is journaled and
does not complete `wait`.

Fault injection `TOOLU_EPIC_FAULT=before-merge|after-merge|before-merge-record`
makes the foreground engine exit 75 at that point.

## Failure modes and edge cases

- Empty registry: `engine --ensure` exits 0 and does not spawn. `status` returns an empty document.
- Engine already live: a second `engine` exits 1 with `engine-busy` and does not delete the owner's lock.
- Stale lock (dead pid): the next start reclaims it.
- Pause file set: the next effect is not called; reports still update snapshots; the file is read again after restart.
- Report with the engine killed after the spool write and before ack: the spool remains and is applied once on start.
- Partial journal line (no trailing newline): ignored.
- Reconcile `unknown`: no second merge call; judgment `failed`.
- Script missing while `TOOLU_EPIC_SCRIPTS` is set: the action is `failed`, not a panic.
- `wait` with no judgment and `--max-seconds 0`: immediate `{"state":"waiting"}`.
- Protocol mismatch on `report`: spool file exists, `replace` stops the old engine at a point where no effect is mid-call, the new engine applies the spool.
- Two issues sharing a worktree: one checkpoint per interval.
- `job` outside a bound worktree: exit 1, diagnostic `worktree has no epic resource binding`, engine not required.
- Note over 1024 characters: truncated. Secret prefix: `[redacted]`.

## Acceptance criteria

- **AC-1:** Given a sandbox git repo, a graph of issue `a` (no blockers) and issue `b` (blocked by `a`), and scripts that launch, merge and clean up, the engine reaches `stage=merged` for both while no client calls `wait`.
- **AC-2:** Given that run's journal, `wait` is never completed by `ready`, `recheck`, `heartbeat` or `host-limited`. A scripted `needs-human` report is the document `wait` returns.
- **AC-3:** Given `TOOLU_EPIC_FAULT` of `before-merge`, `after-merge` and `before-merge-record` in three runs, restarting the engine calls the merge script at most once and still runs cleanup once when the reconcile marker says the merge happened.
- **AC-4:** Given `toolu epic pause`, a following `ready` report does not call merge; after the process is restarted it still does not call merge until `resume`.
- **AC-5:** Given an epic directory written with the TypeScript issue, status and graph keys, the engine rewrites the status file and `plugins/epic-orchestrator/scripts/common.ts` `readJson` still returns `phase` and the original extra key.
- **AC-6:** An idle `toolu epic engine` process with three registered epics, measured two seconds after start, has `VmRSS` of at most 10240 KiB in `docs/toolu/evidence/epic-engine-idle.json`, and its only periodic wake is `TICK` of at least 30 seconds.
- **AC-7:** A second engine start reports `engine-busy`. A checkpoint of a dirty git worktree runs one `read-tree`, `add -A`, `write-tree`, `commit-tree` and `update-ref`, and an immediate restart runs no git. Herdr failure count and retry deadline survive a restart inside the backoff window, probed with a real herdr session.
- **AC-8:** SIGKILL (`kill -9`) of the engine after the report is spooled and before the client receives an ack leaves the spool file in place; the next start applies that report once and the issue phase advances.
- **AC-9:** A client whose protocol is 2 talking to a protocol-1 engine spools the report, the engine exits on `replace`, and a protocol-2 engine applies the spooled report.
- **AC-10:** `wait --max-seconds 0` with no judgment prints `{"state":"waiting"}` and exits 0.
- **AC-11:** `toolu epic job` runs under `run_managed_job` with the engine stopped, without a doubled `--`.
- **AC-12:** `rg` over `crates/epic-orchestrator` finds no new lease store, policy file or `acquire_lease` definition. Calls go to `toolu_engine::resources`.

## Acceptance evidence

- **AC-1:** Input: temp git repo plus `graph.json` and scripts under `TOOLU_EPIC_SCRIPTS`. Check: `cargo test -p toolu-epic-orchestrator scripted_epic`. Expect both issue files `stage=merged` and a journal with no `wait` completion.
- **AC-2:** Same test sends `needs-human` on a third key and asserts the wait body `kind` is `needs-human`. A `host-limited` event is journaled and does not satisfy an earlier `wait` that times out.
- **AC-3:** Input: merge script that writes a marker and counts invocations. Check: `cargo test -p toolu-epic-orchestrator merge_fault`. Expect count 1 and cleanup count 1 for the after-call faults, and count 1 after the before-call restart.
- **AC-4:** Check: `cargo test -p toolu-epic-orchestrator pause_survives`. Expect zero merge invocations across the restart until resume.
- **AC-5:** Input: fixture shaped like `~/.claude/epics/falconiere-toolu-402` issue and status objects, including an extra `agent` key. Check: `cargo test -p toolu-epic-orchestrator adopt_typescript` and the bun read in that test. Expect `phase` updated and `agent` unchanged.
- **AC-6:** Input: three epic directories registered in a temp resource root, release binary `target/release/toolu`. Check: `cargo test -p toolu-epic-orchestrator idle_tick` plus the recorded `docs/toolu/evidence/epic-engine-idle.json` fields `epics=3`, `rss_kib <= 10240`, `tick_secs >= 30`. The test asserts the constant; the evidence file is the resident-set measurement.
- **AC-7:** Input: real `git init` worktree with a dirty file, and `herdr --session` isolated for the probe. Check: `cargo test -p toolu-epic-orchestrator watcher_engine`. Expect the git trace counts and unchanged `herdrRetryAt` on the second load.
- **AC-8:** Input: a spawned engine process. The client writes the spool, then the test sends SIGKILL before reading an ack. Check: `cargo test -p toolu-epic-orchestrator spool_survives`. Expect the spool file to survive the dead process and the next engine to apply that phase once.
- **AC-9:** Check: `cargo test -p toolu-epic-orchestrator protocol_replace`. Expect the spooled phase applied only by the second engine.
- **AC-10:** Check: `cargo test -p toolu-epic-orchestrator wait_limit`. Expect stdout `{"state":"waiting"}`.
- **AC-11:** Input: a git worktree bound with `bind_worktree` and a `true` command. Check: `cargo test -p toolu-epic-orchestrator epic_job`. Expect exit 0 and a journal `admission` line with the engine not started.
- **AC-12:** Check: `rg -n "acquire_lease|policy.json" crates/epic-orchestrator/src` prints no definition; job tests call `run_managed_job`.

## Documentation impact

- `docs/cli/epic.md` and `docs/cli/commands.json` regenerated by `cargo xtask docs-cli`.
- `plugins/epic-orchestrator/README.md` gains the engine verbs.
- `docs/toolu/evidence/epic-engine-idle.json` holds the idle measurement.
- `.toolu/skills/verify-epic-watcher/SKILL.md` gains the engine procedure.
- `crates/cli/src/commands.schema.json` gains the `epicStatus` and `epicWaiting` documents.

## Open Questions

None. Script effects versus a test-only trait was decided (both). Remaining verbs stay on `planned`. Snapshot compatibility is verified by AC-5 rather than left open.
