# Attention notification POST contract

Epic attention notices may go to herdr desktop notifications and to a configured HTTP(S) URL. Set `epic.attention.enabled` to enable URL delivery, then supply `notify_url` in the user-only `secrets.json` or `TOOLU_EPIC_NOTIFY_URL` in the environment. Herdr delivery is a separate, single-attempt channel. An absent URL sends no POST.

## Request and acknowledgment

The URL destination receives a JSON `POST` with `content-type: application/json`. The body has `text`, `kind`, `key`, `epic`, `state`, `note`, `age_seconds`, `reminder`, `issue`, and `dashboard`. Secrets are redacted from the body. There is no authentication header, delivery ID, or receiver deduplication handshake. A URL may carry its own secret; protect it as a credential and use HTTPS outside loopback.

One attempt has a 10-second whole-request deadline and follows no redirects. A 2xx or 3xx response stops retries and is recorded as `notify`; it only establishes that the sender received that HTTP response, not that the receiver durably committed an external effect. A 4xx response other than 429 stops after one attempt and is recorded as `notify-failed`. A timeout, transport error, 429, or 5xx causes up to five attempts total, separated by 1, 2, 4, and 8 seconds. After the last failed attempt, the journal records `notify-failed`. The outcome note is `channel=url attempts=N status=S`; the status is the final HTTP code, `timeout`, `transport`, `refused` for an unusable URL or client setup before sending, or `secrets` when the secret source cannot be loaded.

## Duplicate and loss behavior

The sender accepts **best-effort delivery with possible duplicates** for this informational attention channel. It retries a dequeued notice after an ambiguous response, even if the receiver already committed the first POST. Retries send the same JSON body with no stable delivery identity. A receiver that turns a notice into a side effect may therefore perform that effect more than once. An HTTP success after a retry does not reveal whether an earlier attempt also committed.

This is not a global at-least-once delivery guarantee: the in-memory queue holds 64 notices, drops new notices when full, and has no durable POST replay after process exit. A configured URL may also fail all five attempts. `notify-dropped` records a full or closed queue; no URL yields no URL delivery record. Do not use this channel as an exactly-once command transport.

## Observed fault matrix

The owning crate's tests use a real loopback TLS receiver and CONNECT proxy. They assert request counts and journal outcomes; recording a request before dropping or delaying its response models a committed receiver effect with a lost acknowledgment.

| Receiver situation | Received POSTs | Sender result |
| --- | ---: | --- |
| Invalid URL before send | 0 | `notify-failed`, `attempts=0 status=refused` |
| Invalid secrets before send | 0 | `notify-failed`, `attempts=0 status=secrets` |
| Immediate 404 | 1 | `notify-failed`, `attempts=1 status=404` |
| Immediate 200 | 1 | `notify`, `attempts=1 status=200` |
| Record first POST, drop response, then 200 | 2 identical bodies | `notify`, `attempts=2 status=200` |
| Record first POST, delay response past deadline, then 200 | 2 identical bodies | `notify`, `attempts=2 status=200` |
| Two 500 responses, then 200 | 3 | `notify`, `attempts=3 status=200` |
| Close before recording every request | 0 recorded | `notify-failed`, `attempts=5 status=transport` |

The timeout test shortens the deadline to 200 ms; production uses 10 seconds. The committed-response-loss test initially asserted one received POST and failed with two, then passed with the observed duplicate count. Run `cargo test -p toolu-epic-orchestrator notify::tests -- --nocapture` to repeat the matrix.

## If a receiver needs deduplication

No external receiver or durable acknowledgment capability is identified for this issue. Keep the current wire contract. Before changing it for a side-effecting receiver, agree on a stable ID for each logical delivery, reuse it across retries and restart, give intentional reminders or new notices distinct IDs, and require the receiver to persist the ID atomically with its effect before acknowledging success. The sender must then distinguish a committed but lost response from an explicit refusal during recovery. A sender-only lock cannot establish exactly-once effects.
