# Epic engine core — resident process, state machine, journal

Brainstorm, 2026-10-07. Delivery mode for Falconiere/toolu#434. The 2026-10-04
engine brainstorm stays the product decision; this note records only the
choices #434 has to make so the resident core can land while #435, #446, #447
and #448 are still open.

## Capsule

- **Outcome:** `toolu epic engine` is one resident process per resource root. It
  drives each issue through the phases the TypeScript records already use,
  journals every transition, and wakes an orchestrator only for a judgment
  event. Scripted workers in a sandbox repository go from launch to merged and
  cleaned up with nobody calling `toolu epic wait`.
- **Material defaults:**
  - Consume `toolu_engine::resources` from #421. No second lease, policy or job
    runner. `toolu epic job` calls `run_managed_job` and works with the engine
    stopped. One `--` separates the command; the TypeScript doubled `--` is not
    copied.
  - Real clap verbs for #434 (`engine`, `start`, `status`, `pause`, `resume`,
    `ack`, `answer`, `wait`, `report`, `job`, plus `service` and the existing
    `token`). `planned` stays and lists only #435 and #448.
  - Effects are a library trait. When `TOOLU_EPIC_SCRIPTS` is set, the CLI runs
    those programs for merge, cleanup, launch, reconcile and prompt. When it is
    unset, the action is journaled as deferred for #435 and #448 and is not
    performed.
  - Snapshots keep every existing JSON key. Status files gain the same `phase`,
    `pr`, `note`, `updated_at` and `history` shape `report.ts` writes.
  - Control socket protocol is 1, under `<resource-root>/engine.sock` mode
    0600. A greeting line carries the protocol. On mismatch the client spools
    the report and sends a version-stable `replace`; the engine drains at a
    safe point and the CLI starts the new process.
  - Idle tick is 30 seconds. Checkpoint interval stays 15 minutes. Herdr
    backoff stays 30 seconds doubling to 5 minutes. No timer shorter than 30
    seconds.
- **Non-goal:** GitHub polling (#447), herdr subscriptions (#446), the real
  graph, route, launch and finish actions (#435), the merge queue (#448), the
  HTTP status server, hub doctor/config/status/setup (#445), and pre-tool gates
  (#419).
- **Repository evidence:** `docs/toolu/brainstorms/2026-10-04-epic-engine.md`,
  issue #434, `toolu_engine::resources::jobs::run_managed_job`,
  `resource_home`, the planned verb list in `crates/epic-orchestrator`, and
  Jev (`jev-1.13.0`): effects `both` 0.86, CLI `real-plus-planned` 1.0,
  snapshot preservation 0.62.
- **Risk:** snapshot preservation is only moderately supported. The adoption
  test must run the TypeScript `readJson` path on an engine-written directory.
  Unattended merge in production stays deferred until #448; the sandbox scripts
  are the acceptance stand-in and must be idempotent under reconcile.
- **Handoff:** spec.

## Axes

- **Intent.** The engine performs deterministic transitions itself. Sources and
  actions plug in through the script directory and a socket `event` for `gone`,
  `blocked` and `host-limited`. Judgment events are only `needs-human`,
  `failed`, `blocked`, stall after one `STATUS?`, `dependency-cycle`,
  `coupling-hold`, `cleanup-incomplete` and `relaunch-limit`.
- **Data.** Source of truth on disk is the TypeScript snapshots plus an
  append-only JSON-lines journal under `<resource-root>/journal/`. Recovery
  replays the journal tail, then reconciles an open merge before calling it
  again. Unknown reconcile results are not retried.
- **Interface.** Unix socket, mode 0600. `wait --max-seconds` defaults to 480
  on Codex, OpenCode, Cursor and Hermes, and 2700 on Claude. At the limit the
  document is `{"state":"waiting"}`.
- **Failure.** Pause is a file and is checked before every effect. A report
  that does not receive an ack stays in the spool and is ingested on start.
  Worker notes are capped at 1024 characters and a recognized token prefix is
  replaced with `[redacted]`.
- **Integration.** Resource root is `resource_home`. Detach uses
  `toolu_runtime::process` so the plugin crate does not spawn processes itself.
- **Horizon.** #435 and #448 replace the script directory with in-process
  actions without changing the journal or the socket greeting.
