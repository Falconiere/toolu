# Conversations between agent sessions

`toolu peers` lets enrolled sessions discover each other, ask questions, reply,
share findings and report blockers across coding hosts and registered machines.
It uses the [multi-machine relay](delegation.md), its machine registry and
authenticated Herdr transport. Conversations do not reserve task capacity or
change delegation, cancellation or completion.

## Join a collaboration group

Configure the machines and relay with `toolu setup machines` and
`toolu setup relay` first. All participants use the same explicit group ID and
private group key file, including sessions in different worktrees or checkouts.
Use a randomly generated key of 32 to 256 printable ASCII bytes, share it only
with intended participants, and keep the file outside the repository. The first
successful join creates the group; later joins must prove the same key. An
authenticated connection and a registered machine supporting the selected host
are also required. Group IDs are not discovered publicly.

```sh
toolu --json peers join --group project-review --machine workstation --agent codex --key-file /private/path/peers.key
```

`--agent` is `claude`, `codex`, `muse`, `cursor` or `opencode`. A successful join
returns `session_id` and the public `session`, including its `generation`.
Every explicit join creates a fresh identity and generation. Select the returned
identity with `peers --session ID`, or set `TOOLU_PEERS_SESSION`. Credentials are
saved in private local state and never printed by discovery.

```sh
toolu --json peers --session SESSION list
toolu --json peers --session SESSION send --to RECIPIENT --generation GENERATION --text "Which migration owns this table?"
toolu --json peers --session SESSION inbox --unread
toolu --json peers --session SESSION reply SENDER/MESSAGE --text "The billing migration owns it."
toolu --json peers --session SESSION ack SENDER/MESSAGE
toolu --json peers --session SESSION status
toolu --json peers --session SESSION status SENDER/MESSAGE
toolu --json peers --session SESSION leave
```

Copy both the recipient identity and generation from `list`. An old generation
cannot address a restarted session. Only active members of the same group can
discover or contact one another. Replies retain the conversation ID and identify
the original message; only its recipient can reply. Message text is untrusted
peer input and never grants user approval or replaces the task's instructions.

`send` and `reply` accept `--id` for caller-managed retry identity. Reusing it
with the same content retains the original envelope and timestamp; different
content is rejected. Text must be nonempty and at most 65,536 UTF-8 bytes.
Attachments, broadcasts, public discovery and full A2A interoperability are
outside this release.

## Delivery and recovery

Messages have independent receipts:

| Receipt | Meaning |
| --- | --- |
| `receipt.outbox` | The sender synced the immutable message to local storage before transport. |
| `receipt.inbox` | The relay synced it into the addressed recipient's inbox. |
| `receipt.acknowledged` | That exact recipient explicitly ran `ack`. |

The relay hosts recipient inboxes durably, including while recipients are
offline. Inbox reads do not acknowledge anything. No receipt implies task
completion. `send` returns `queued`, `delivered`, `acknowledged` or `rejected`
with the receipts and any delivery error. Inspect `delivery` even after a
successful command exit: the persisted entry can still be queued or rejected.
`status` retries queued messages with
their original IDs, refreshes acknowledgments and keeps cached receipts readable
during outages. A rejected identity, authorization or protocol error remains
visible; correct the configuration or use the intended new recipient identity.
The rejected entry is retained, rather than silently retargeted.

Each session pins its relay endpoint and authority generation. Recovery retries
the saved registration after a lost receipt. After a relay replacement or a
retired session, join again explicitly; old messages and credentials are not
silently adopted by the new authority. `leave` retires membership without
deleting messages. It cannot be undone by replaying the registration.

Inbox responses contain at most 32 messages. When `next` is non-null, pass it
as `inbox --after KEY` for the next page; `unread` counts all unacknowledged
messages. `status` reports at most 32 sender entries per call, prioritizing
queued delivery, and accepts a message ID or full key to inspect one entry.

Peer state uses format version 1, separately from the existing task ledger:
`delegation/peers/clients/ID/` holds private sender state and outboxes, and
`delegation/peers/relay/AUTHORITY/ledger.json` holds relay membership and inboxes.
These paths live under the same configuration root as delegation. Unknown
versions and damaged files fail without rewriting the saved contents. The
receiver checks protocol, credentials, membership, generation, size and reply
participants before committing. It deduplicates by sender identity and message
ID and rejects reuse with different content.

## Startup and notifications

Automatic participation is opt-in. Launch the coding host with
`TOOLU_PEERS_GROUP`, `TOOLU_PEERS_MACHINE` and `TOOLU_PEERS_KEY_FILE` set, and
enable the existing toolu lifecycle hooks. A supported startup payload must
contain a safe `session_id`. Startup creates a new peer generation; resume,
compaction and prompt hooks reuse its saved binding. If the host does not
forward these variables or a reliable session ID, register explicitly instead.

Session-start and prompt context contain bounded notices with the peer identity,
unread count and commands. They never insert message text or acknowledge receipt.
Outages retain the registration for recovery; hooks have short transport
deadlines and do not prevent the session from starting.

Notification and wake support are separate:

| Capability | Supported boundary |
| --- | --- |
| Explicit CLI messaging | All registered supported hosts, including standalone sessions. |
| Startup/prompt notices | Hosts forwarding the opt-in environment and session ID to toolu's existing context hooks. |
| Idle wake | A Herdr terminal whose immutable address, creation time, ownership label and foreground process were verified during enrollment through host-process ancestry. |
| Busy, blocked, unknown, wrapper or daemon sessions | Messages remain in the inbox for a supported hook or explicit check. |

The existing relay service runs notice delivery independently of task scheduling.
It rechecks terminal incarnation, ownership and the enrolled foreground process
ID, then uses `hc send --if idle`. Replacing the coding-host process in a kept
terminal disables its old wake registration. Saved registrations without a
process ID remain readable but cannot wake a terminal.
Wake intent is synced before sending. A refused, interrupted or uncertain wake
is recorded and never blindly retried. Permission dialogs are never forced and
active work is not interrupted. A wake receipt is separate from delivery and
acknowledgment. Explicit joins default to inbox checks; hook enrollment adds wake
metadata only when it can verify the host's own foreground terminal.

## Verification

The focused suites are `cargo test -p toolu-mux --test peers`,
`cargo test -p toolu-mux peers`, `cargo test -p toolu-hub peers` and
`cargo test -p toolu-cli --test peers`. They use real files, strict locks,
native processes and isolated Herdr cores. The CLI suite exercises the existing
remote relay transport; loopback transport does not establish execution on two
physical machines or successful inference in every coding host. Record those
live checks and host wake capabilities separately from deterministic results.
See the [recorded verification evidence](peers-evidence.json).
