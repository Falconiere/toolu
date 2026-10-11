# Epic attention queue and notifications

Brainstorm, 2026-10-11, delivery mode, full path. Issue #450, part of epic #402.

## Capsule

- **Outcome:** a judgment event (`needs-human`, `failed`, `blocked`, a stall after its nudge, `coupling-hold`, `dependency-cycle`, `cleanup-incomplete`, `relaunch-limit`, and the engine's own `babysit`, `pr-closed` and `herdr-error` items) becomes one attention item with a key, note, age and state (`open`, `acknowledged`, `answered`). It reaches `toolu epic wait`, a herdr notification, an optional outgoing URL and the dashboard within seconds. `toolu epic answer` prompts the worker and marks the item answered; the worker's next report closes it and the journal records how long it waited.
- **Material defaults:**
  - The queue stays in the engine's `World`, rebuilt after a restart from the journal tail, like pending actions.
  - One live item per key and kind: a repeat updates the note and does not notify again.
  - Any later worker report for the key closes the key's items; a judgment report then opens a fresh one.
  - Reminders go to the herdr notification and the URL only, first after 30 minutes (`epic.attention.remind_after_minutes`), then at most hourly. `wait` returns an item once.
  - The notifier is its own thread with a bounded queue, so a slow or failing URL never blocks the state loop.
  - The URL stays the `notify_url` secret of #463, used only when `epic.attention.enabled` is true. The payload carries no secret and no pane text.
  - A `blocked` item carries the agent's last screen lines as `pending`, shown only by `wait` over the 0600 control socket.
- **Non-goal:** `toolu epic release` and the coupling hold it clears (#435), rewriting the skill around the engine (#452), metrics over the journal (#451), and any HTTP write surface.
- **Repository evidence:** `crates/epic-orchestrator` already has `wait`, `ack` and `answer` socket ops, `Attention` items and `/api/v1/attention` (#434, #449). `answer` only journals today, `ack` only touches stalls, nothing closes an item, a waiter's delivery hides the item from the dashboard, and the queue is lost on restart. `toolu_runtime::config::{epic, secrets}` (#463) already rejects `notify_url` in config and resolves it from `secrets.json` or `TOOLU_EPIC_NOTIFY_URL`. herdr 0.9.3 (protocol 22) has `notification.show {title, body?, position?, sound}`.
- **Risk:** the outgoing URL is a secret and could leak through an error message; a burst of events against a dead URL could queue without bound; reminders could wake the orchestrator LLM for nothing.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Persistence | `World.attention`, rebuilt from `judgment` journal records that carry an item token | `recover::observe` already rebuilds pending actions from the journal tail | Items older than the two-day tail are not rebuilt |
| Interface | `wait` and `toolu epic status` keep their fields and gain `state` and `age_ms`; `wait` gains `pending` for `blocked`; `answer` gains `--by` | `epicStatus` schema in `crates/cli/src/commands.schema.json` | Consumers that reject unknown fields; the schema is updated with them |
| Security | URL read per notice from the secret store; payload note passes the journal redaction and `secrets::redact_text`; failures journal an error class, never a message | #463 spec, HTTP status design ("no pane transcripts") | A secret embedded in a worker note is caught only by the known prefixes and the resolved secrets |
| Cost | One POST per new item and per reminder; at most five attempts with 1, 2, 4, 8 s backoff; queue of 64, overflow dropped and journaled | Slack and ntfy accept a JSON POST | A dropped notice is visible only in the journal and the dashboard |

## Alternatives

| Option | Verdict | Why |
|---|---|---|
| Replace the skill's watcher table with the judgment table now | Rejected | Live runs still use `epic-watch.ts`; #452 rewrites the skill once #435 and #448 land. Jev 1.00 for adding a separate engine table. |
| Name `toolu epic release` in the table | Rejected | It is a `planned` verb (#435) and the Markdown–CLI gate fails on it; named in prose instead. |
| Reminders also re-deliver to `wait` | Rejected | The epic target is zero orchestrator wake-ups without a new judgment; the LLM already handled the item once. Jev 0.57 against 0.43, so this rests on that target. |
| Close only answered items on the next report | Rejected | A worker that recovers on its own would leave a stale item. Jev 0.72 for any later report. |
| Pending command in the HTTP API and the outgoing notification | Rejected | The status server and the outgoing payload must not carry pane text. Jev 0.66 for `wait` only. |
| Send notifications from the state loop | Rejected | A slow URL would stall merges and reports. |
| Store the queue in its own file | Rejected | The journal already holds every raise and close; a second store could disagree with it. |
