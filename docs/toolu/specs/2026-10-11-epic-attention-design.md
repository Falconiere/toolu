# Epic attention queue and notifications — Design

**Date:** 2026-10-11   **Status:** Approved   **Author:** epic worker (Cursor)   **Topic:** #450, judgment events as a queue with answers, acknowledgements, reminders and notifications

Brainstorm: `docs/toolu/brainstorms/2026-10-11-epic-attention.md`.

## Problem

`needs-human` is 16% of measured epic wall time. The engine (#434) already turns judgment events into attention items, but only `toolu epic wait` sees them. Nobody away from the keyboard hears about one. `toolu epic answer` journals the text and never reaches the worker. Nothing closes an item, a waiter's delivery hides it from the dashboard, and an engine restart forgets it. A question can therefore wait unseen for hours.

## Non-Goals

1. No `toolu epic release` and no coupling-hold release: the verb stays `planned` for #435.
2. No rewrite of the skill around the engine: #452 does that. This issue adds the engine's judgment table and keeps the watcher table that live runs use.
3. No journal metrics or budgets (#451); this issue writes the records they read.
4. No HTTP write surface: answer and ack stay CLI verbs over the 0600 control socket.
5. No change to the authorization boundary: approving a `blocked` prompt stays with the orchestrator LLM or the user.

## Architecture

The queue stays in `World.attention`; a new `attention` module owns it. Every raise goes through `attention::raise`. `logic::push_attention` keeps its signature and calls it. The module also owns close-on-report, ack, answer, reminders and the rebuild from the journal.

- **Identity and dedup.** Each item has a token `a<raised_ms>-<seq>`. A raise for a key and kind that already has a live item updates its note and sends nothing new. A stall past its full clock or a new `blocked` spell is a new event: it reopens an item a waiter already saw, or one acknowledged or answered, journals `reopened` and announces it again.
- **States.** `open`, `acknowledged` (`toolu epic ack`), `answered` (`toolu epic answer`). Closed items leave the queue.
- **Close.** A worker report for a key closes every live item of that key before the report's own judgment, if any, is raised. Each close is journaled with the kind and the wait in milliseconds.
- **Delivery to `wait`.** `wait` returns each item once (`delivered`). The status overview, `toolu epic status` and `/api/v1/attention` list every live item until it closes, with its state and age.
- **Notifications.** A raise or a reminder pushes `Step::Notify`. The engine builds a notice from the item and the issue snapshot (`title`, `url`) and hands it to the notifier thread over a bounded channel (64). A full channel drops the notice and journals `notify-dropped`. The thread does two things per notice:
  - a herdr `notification.show` when herdr is enabled;
  - a POST to `notify_url` when `epic.attention.enabled` is true and the secret is set. The URL is resolved per notice through `toolu_runtime::config::secrets::load`.

  The POST uses `toolu-http` with a 10 s deadline. Up to five attempts are made, 1, 2, 4 and 8 s apart. A transport error, a timeout, 429 or 5xx is retried; any other status of 400 or more fails at once. The state loop never waits on the thread.
- **Reminders.** On each engine tick, an `open` item announced `remind_after_minutes` ago (default 30) is announced again, then at most once an hour. Reminders reach the notifier only, never `wait`. Acknowledged and answered items get none. The socket's wake deadline includes the next reminder.
- **Answer.** The `answer` op requires a live item for the key. It sends the text through the existing worker prompt path (herdr `agent.prompt`, or the `prompt` script under `TOOLU_EPIC_SCRIPTS`). Only after delivery does it journal the answer and mark the key's live items answered, noting who answered. A failed delivery is an error to the caller, and the item stays as it was.
- **Blocked.** When herdr reports `blocked`, the engine always reads the agent tail, even when the phase was already scanned. It stores the last 10 non-empty lines, at most 1024 characters, as the item's `pending`, behind the same token-prefix redaction as notes. Only `wait` shows `pending`.
- **Rebuild.** `recover::observe` passes each journal record to `attention::observe`. A `judgment` record with an item token reopens the item, with its raise time taken from `at`. `acknowledged`, `answered`, `reminder` and `closed` records then replay the item's state, its last announcement and its reminder count. A rebuilt item sends no notification. It counts as undelivered, so `wait` may return it once more after a restart.

Reused: `Step::Prompt`'s send path (`Engine::send_prompt`), `herdr::Client`, `toolu-http`, `secrets::{load, redact_text}`, `note::bounded_note`, `journal::append`, and `toolu_state::time::parse_iso`.

## Interfaces / Schema

Config (`toolu.config.json`, `epic` namespace, read by `toolu_runtime::config::epic::parse`):

| Key | Type | Default | Meaning |
|---|---|---|---|
| `epic.attention.enabled` | boolean | `false` | Existing (#463): send the outgoing POST |
| `epic.attention.herdr` | boolean | `true` | Show a herdr notification |
| `epic.attention.remind_after_minutes` | integer 1–10080 | `30` | First reminder delay; later reminders are hourly |

`EpicSettings` gains `attention_herdr: bool` and `remind_after_minutes: u64`. A wrong type or a value out of range fails with a path-only error, as the existing keys do.

Journal records (closed keys unchanged):

| kind | name | token | note |
|---|---|---|---|
| `judgment` | item kind (`needs-human`, …) | item token | the bounded note |
| `judgment` | `acknowledged` | item token | `by=<who>` |
| `judgment` | `answered` | item token | `by=<who>` |
| `judgment` | `reminder` | item token | `kind=<kind> reminders=<n>` |
| `judgment` | `reopened` | item token | the bounded note |
| `judgment` | `closed` | item token | `kind=<kind> waited_ms=<n> by=report` |
| `prompt` | `answer` | item token | the answer text |
| `action` | `notify` / `notify-failed` / `notify-dropped` | item token | `channel=<herdr\|url> attempts=<n> status=<code\|timeout\|transport\|refused\|secrets>` |

`wait` document: `{"kind","key","epic","note","seq","state","age_ms"}`, plus `"pending"` for a `blocked` item when screen lines were read. `{"state":"waiting"}` is unchanged.

`toolu epic status` (no argument) and `/api/v1/overview` `attention[]`: `{"kind","key","epic","note","seq","state","age_ms"}` for every live item. `/api/v1/attention` `items[]` have the same fields, with the note cut to 160 characters. `attention_count` counts live items. `pending` never appears in these documents.

Outgoing POST body (`content-type: application/json`):

```json
{"text":"toolu epic: falconiere-toolu-450 needs-human: <note> <issue url> <dashboard>",
 "kind":"needs-human","key":"falconiere-toolu-450","epic":"falconiere-toolu-402",
 "state":"open","note":"<note>","age_seconds":0,"reminder":false,
 "issue":{"title":"…","url":"https://github.com/…/issues/450"},
 "dashboard":"http://127.0.0.1:7717/"}
```

`issue` is `null` without an issue snapshot. `dashboard` is `null` when the engine runs with `--no-http`; an unspecified bind address (`0.0.0.0`, `::`) is shown as `127.0.0.1`. Slack incoming webhooks read `text`.

herdr call: `notification.show {"title":"toolu epic: <key> <kind>","body":"<note, ≤200 chars>","sound":"request"}`. The CLI fallback for a newer protocol is `herdr notification show <title> --body <body> --sound request`.

CLI:

- `toolu epic answer <key> <text> [--by NAME]` prompts the worker, marks the item answered and journals who answered. `--by` defaults to the `--host` name (`claude`, `codex`, `cursor`, `opencode`, `hermes`), else `user`; environment detection always names a host, so it would record a person at a terminal as `claude`.
- `toolu epic ack <key>` acknowledges every live item of the key. It still resets the stall clock, so the stall is not raised again until it has run its full length once more. An item with no key (a dependency cycle) is acknowledged by its kind: `toolu epic ack dependency-cycle`.

Rust: `model::Attention` gains `raised_ms`, `state: Ack` (`Open`, `Acknowledged`, `Answered`), `announced_ms`, `reminders`, `token` and `pending: Option<String>`. `World` gains `remind_ms`. `Step` gains `Notify { token: String, reminder: bool }`. `socket::ServeOptions` gains `attention: AttentionOptions { remind_ms, notify: Option<notify::Settings> }`. New modules: `attention.rs`, `notify.rs` (thread, POST, retry) and `notify_notice.rs` (notice and payload), each with `tests/<module>_test.rs`.

## Failure modes and edge cases

- **No item for the key:** `answer` returns `{"error":"no open attention for <key>"}`, and the CLI exits 1. An empty key or empty text is also an error. The item is not touched.
- **Prompt not delivered** (herdr down, prompt script `failed` or `deferred`): `answer` fails with `worker prompt not delivered: …`. The item stays `open` or `acknowledged`, and nothing is journaled as answered.
- **No URL** (`enabled` false, no secret, or a malformed `secrets.json`): no POST. A secret-load error journals `notify-failed status=secrets`, and herdr delivery still happens.
- **URL refused by `check_url`:** `notify-failed status=refused` with no attempt.
- **Failing URL:** retries as above. Once they are spent, `notify-failed` with the last class. The state thread keeps serving the socket the whole time.
- **Burst over 64 queued notices:** the extra notices are dropped with `notify-dropped`, and their items stay in the queue and the dashboard.
- **herdr unavailable or disabled** (`TOOLU_EPIC_HERDR=off`, tests by default, `epic.attention.herdr=false`): no herdr call. A herdr failure journals `notify-failed channel=herdr`.
- **Repeated raise** (babysit escalating every tick): one live item; the note updates; no second notification.
- **Report with the same judgment phase** (`needs-human` twice): the first item closes and a new one opens with the new note and a new notification.
- **Restart:** live items come back from the journal tail, which covers today and yesterday; older items are not rebuilt. Records without an item token, written by engines before this change, are ignored.
- **Clock skew:** a raise time after `now` counts as age 0.
- **Secrets in notes:** the known token prefixes become `[redacted]` (existing). Resolved secret values become `<redacted>` in the POST body and the herdr body.
- **Cycle items** (`key` empty): acknowledged by kind (`ack dependency-cycle`); no report closes them, so they stay listed until the engine restarts past the journal tail.
- **Items whose worker is gone** (`cleanup-incomplete`, `relaunch-limit`, `pr-closed`): they stay listed until a report for the key arrives. `ack` stops their reminders.
- **Ack of an unknown key:** `{"ok":true}` with nothing changed, as today, so a repeated ack is harmless.

## Acceptance criteria

- **AC-1:** Given a running engine with HTTP on a loopback port and the notify URL set to an HTTPS loopback fixture, a `needs-human` report for an issue with a snapshot `url` produces, within 2 seconds:
  - a `wait` document with `kind=needs-human` and `state=open`;
  - a `/api/v1/attention` item for the key with an `age_ms`;
  - exactly one POST whose JSON has the kind, key, note, issue URL and dashboard URL, and contains no planted secret canary.
- **AC-2:** Given an open `needs-human` item, `toolu epic answer <key> "<text>" --by user` delivers the text to the worker's prompt script once. The item shows `state=answered`, and the journal has `prompt answer` and `judgment answered by=user`. The worker's next report (`execution`) removes the item and journals `judgment closed kind=needs-human waited_ms=<n>`. `answer` for a key with no live item fails and calls no prompt.
- **AC-3:** Given an open item, a 30-minute reminder setting and no waiter:
  - nothing at 29 minutes;
  - exactly one reminder notice and one `reminder` record at 30 minutes;
  - none again until 90 minutes, the next hourly mark;
  - none for an acknowledged item;
  - the item stays listed throughout, and `wait` returns it once.
- **AC-4:** Given a notify URL answering 500, 500, then 200, the fixture sees three POSTs and the journal has `notify` with `attempts=3`. Given a URL that drops every connection, the journal has `notify-failed` after five attempts. In both cases a `status` request to the engine during the retries answers in under 500 ms.
- **AC-5:** After `needs-human` and `stall` items are each raised and closed, every `closed` journal record names its kind and a non-negative `waited_ms`, and the same token links it to its raise record.
- **AC-6:** Given a herdr `blocked` status for a running issue whose screen tail ends with an approval prompt, the `wait` document's `pending` holds those lines, and neither `/api/v1/attention` nor the status overview contains them.
- **AC-7:** An open item and an acknowledged item survive an engine restart with their kind, note and state, and a closed item does not come back.
- **AC-8:** With herdr enabled against a scripted herdr socket, a raise sends one `notification.show` with the title `toolu epic: <key> <kind>` and `sound` `request`.
- **AC-9:** `epic.attention.herdr` and `epic.attention.remind_after_minutes` parse with their defaults, reject a wrong type or a value out of range with a path-only error, and still reject a `notify_url` key.
- **AC-11:** After `toolu epic ack <key>` on a raised stall, ticks before the full stall length has passed again raise no second stall item and no reminder, and the item shows `state=acknowledged`.
- **AC-10:** The skill has an engine attention table that lists only judgment events and names `toolu epic wait`, `toolu epic answer`, `toolu epic ack` and `toolu epic pause`. `cargo xtask check-markdown-cli` passes, and `docs/cli/` and the command snapshot match the binary.

## Acceptance evidence

| AC | Real input → expected result | Boundary / failure | Runnable check |
|---|---|---|---|
| AC-1 | A temp resource root, a real engine thread (`serve_with_http`) with HTTP on `127.0.0.1:0`, an `issues/a.json` with `url`, `TOOLU_EPIC_NOTIFY_URL` set to a `toolu-http-test-support` fixture URL, a canary in `TOOLU_EPIC_STATUS_TOKEN` and in the note. Expect the three results and the timing. | The canary is absent from the POST | `cargo test -p toolu-epic-orchestrator attention_reaches_every_channel` |
| AC-2 | The same engine with `TOOLU_EPIC_SCRIPTS` and a `prompt` script that appends its stdin to a file. Expect the prompt body once, the states, and the journal rows. | No live item: error and no prompt call; failing prompt script: item stays open | `cargo test -p toolu-epic-orchestrator answer_` |
| AC-3 | A real `World` with a clock moved by `now_ms`; collect the outbox steps. | An acknowledged item; the hourly cap | `cargo test -p toolu-epic-orchestrator reminder_` |
| AC-4 | The fixture's `sequence` of 500, 500, 200 replies, then dropped connections, with the test's retry delays shortened through `notify::Settings`. | Dropped connections; the engine stays responsive | `cargo test -p toolu-epic-orchestrator notify_retries` |
| AC-5 | The AC-2 engine journal plus a stall raised by `on_tick` and closed by a report | The token links raise and close | `cargo test -p toolu-epic-orchestrator closed_records_` |
| AC-6 | `limit::on_status` on a real `Engine` with a screen tail containing `Do you want to run rm -rf build? (y/n)` | The HTTP documents lack the text | `cargo test -p toolu-epic-orchestrator blocked_pending` |
| AC-7 | An engine journal with raise, acknowledge and close records; `snapshot::load` on the same root | Records without a token are ignored | `cargo test -p toolu-epic-orchestrator attention_rebuild` |
| AC-8 | A `UnixListener` speaking herdr protocol 22 (as `herdr_test.rs` does) that records methods | herdr disabled: no connection | `cargo test -p toolu-epic-orchestrator herdr_notification` |
| AC-9 | `LoadedConfig::from_data` with valid, default and invalid `epic.attention` values | Range 0 and 10081; string type | `cargo test -p toolu-runtime --lib -- config::epic::` |
| AC-11 | A real `World` with a running issue whose phase passed the stall limit after its nudge; `schedule::ack`, then `on_tick` at later clocks | Before and after the full stall length | `cargo test -p toolu-epic-orchestrator ack_silences_` |
| AC-10 | The edited Markdown and the rebuilt binary | A planned verb named as a command fails the gate | `cargo xtask check-markdown-cli`; `cargo xtask gate --only docs-cli`; `cargo test -p toolu-cli --test contract` |

## Documentation impact

- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`: an engine attention table, next to the watcher table that stays until #452.
- `plugins/epic-orchestrator/README.md`: the `ack`, `answer` and `wait` rows, and notifications and reminders.
- `docs/config.md`: the `epic.attention.*` keys.
- `docs/cli/epic.md` and `docs/cli/commands.json`: regenerated with `cargo xtask docs-cli`.
- `docs/epic-status-api.md`: the attention fields.
- `crates/cli/src/commands.schema.json`: `epicStatus.attention` items and a new `epicJudgment` document.

## Open Questions

None blocking. Decisions taken from the issue, the epic and the code, with Jev where the choice was semantic:

- The skill keeps the watcher table and adds an engine table (Jev 1.00), because live runs still use `epic-watch.ts` and #452 owns the rewrite.
- `release` is named in prose as planned in #435, because the Markdown–CLI gate rejects a planned verb used as a command.
- Reminders go to notifications only (Jev 0.57, low confidence). This rests on the epic's target of zero orchestrator wake-ups without a judgment.
- Any later report closes the key's items (Jev 0.72).
- `pending` appears in `wait` only (Jev 0.66), because the HTTP server must not serve pane text.

## Spec review

Round 1. Jev scored issue coverage 2.37 out of 3 (0.60 on full coverage) and was uncertain about unjustified deviation (0.39); it saw the criteria only, while the skill-table deviation is justified under Non-Goals and Open Questions. The agent's own pass against the code found four gaps, all fixed in place:

- Interfaces: 🟡 should-fix: acknowledging a keyless cycle item was undefined. It is now acknowledged by kind.
- Architecture: 🟡 should-fix: the rebuild did not replay reminders, so the hourly cap could reset after a restart. `reminder` records now replay.
- Failure modes: 🔵 consider: items whose worker is gone, and an ack of an unknown key, were unstated. Both are stated now.
- Acceptance: 🟡 should-fix: "ack silences repeats of a stall" had no criterion. AC-11 covers it.

No blocker remains. **Status:** Approved.
