# Epic status server and dashboard (#449)

## Capsule

- **Outcome:** One resident engine exposes versioned read-only status documents, a replayable event stream, and an embedded dashboard. `toolu epic status` reads the same documents over the local control socket.
- **Material defaults:** The state thread remains the sole writer. HTTP readers use its published `World` snapshot, the journal, registered epic snapshots, and the shared resource store. Every public document is built from an allow-list of fields. The listener is on by default at `127.0.0.1:7717`; `epic serve` runs the foreground engine, while `--http` and `--no-http` control its listener.
- **Non-goal:** HTTP mutations, fleet aggregation (#453), metrics (#451), notifications (#450), or implementing the merge actions owned by #448. Status can display an empty queue until #448 supplies entries.
- **Repository evidence:** `socket.rs` already publishes a cloned `World` to `Arc<Mutex<World>>` after each state step; `journal.rs` stores ordered JSON lines; `snapshot.rs` reads registered `graph.json`, issue and status files; `toolu-engine::resources` owns the machine policy, pressure and leases. Real epic #402 snapshots include `brief`, `excerpt`, `note`, `history`, routes and agent session metadata, so raw serialization would leak fields outside the public contract. `docs/resource-budgets.md` sets 7 ms p90 and 13 MiB RSS for 50 keep-alive clients.
- **Risk:** Concurrent readers must never delay state transitions, streams must replay across reconnects and daily journal rotation, and private source fields must not enter an error or event response.
- **Handoff:** Specify the response types, auth boundary, bounded listener and real-process acceptance tests.

## Decisions

| Axis | Decision and evidence | Remaining risk |
|---|---|---|
| Live state | Read the published in-memory snapshot under a short lock; read journal and bounded disk metadata outside the lock. The control socket builds the same document shape. | Files may change between two reads; each response identifies its journal sequence so callers can refresh. |
| Events | Use the journal sequence as the SSE `id`. Reconnect reads every retained record after `Last-Event-ID` or `since` in order, then waits for a state-change notification. Heartbeats keep the connection live. | Retention is finite; the API states the oldest available sequence. |
| Connection model | A bounded connection cap with small blocking worker threads and read/write deadlines; only eight streams can remain open. Reject excess work promptly. No async runtime. | The 50-client latency and RSS budget must be measured on Linux. |
| Public data | Construct JSON from explicit safe fields. Truncate worker notes; omit journal tokens, brief and graph excerpts, raw environment, transcript and notification URLs. Apply the configured secret redactor to the final public document. | New source fields stay invisible until deliberately mapped. |
| Security | Default loopback. A non-loopback bind requires the status token from #463, compared in constant time. Reject foreign `Host`, disallow all methods but `GET` and `HEAD`, send no CORS headers, and use `no-store` on every response. | Header parsing and connection limits need direct socket tests. |
| CLI | `epic status [<epic>|<key>]` remains a control-socket query. Add a document selector for the same public views; preserve the current summary where practical. | Existing consumers may expect its minimal fields, so retain those fields. |

## Alternatives

- **HTTP through the control socket for every request:** rejected because the single state thread would serialize 50 polling clients and could wait behind disk reads.
- **Reconstruct state from the journal for every request:** rejected because the existing `World` snapshot already contains current transitions and the journal spans daily files.
- **Serialize source JSON directly:** rejected because actual issue and graph files include brief paths, excerpts and worker notes outside the response contract.

Jev could not score these alternatives in this worktree: the installed `toolu jev` contains only the `planned` subcommand. The choices above follow the source and the issue's latency and privacy constraints.
