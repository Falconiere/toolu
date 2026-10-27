# Dashboard worker logs — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Codex   **Topic:** Follow worker background output from the local dashboard.

## Problem

The worker screen shows only pane-visible text. Managed ledger checks and Claude Code background Bash commands can run without displaying their output there. Operators need a bounded, token-gated follow view without attaching to the pane.

## Non-Goals

1. No peer proxy, push stream, arbitrary redirected files, or Codex/Muse private terminal discovery.
2. No change to the returned `toolu_runtime::process::Output` fields or to existing process callers.
3. No new dependency or shell wrapper in product code.

## Architecture

`toolu_runtime::process` adds a chunk observer to its existing stdout and stderr drains; `run_with` keeps its API by calling the new variant without an observer. `run_managed_job` installs an observer after acquiring a lease. Its output chunks are serialized into `<resource_root>/jobs/logs/<lease token>.log` before the returned-output budget discards bytes. A per-token JSON sidecar holds the issue key, command label, start time and retained-window base offset. A separate lock file protects log, sidecar and ring wrap operations across the worker CLI and HTTP listener processes. The file retains the last 8 MiB; logical offsets count all bytes emitted. A logging failure is reported, never silently treated as a successful managed job.

The local HTTP listener combines job sidecars with regular Claude `.output` files discovered only below `/private/tmp/claude-<uid>/<worker worktree slug>/<session>/tasks/`. It excludes symlinks, verifies the issue exists locally, and resolves detail IDs from that issue's fresh listing. Running job state comes from current leases; a finished job remains listed until retention. A Claude file is `running` when the issue stage is `running` and its modification time is at most three seconds old; otherwise it is `done`. This is approximate because the observed private directory has no completion marker. Missing worktrees suppress the Claude source. Each new job and each log listing prunes completed jobs across all keys older than 24 hours and those beyond the 20 newest for each key.

The existing drawer keeps Plan and Phases visible. Screen and Logs become two accessible tabs. The Logs view polls every second only while selected, the drawer is open, and the browser tab is visible. It follows `next_offset`, keeps the bottom pinned while the reader is there, offers Jump to latest after scrolling up, and strips ANSI controls before rendering with text APIs. Only `js/api.js` calls `fetch`.

## Interfaces / Schema

`GET /api/v1/issues/{key}/logs` returns `{"schema":1,"items":[{"id":"…","source":"job|claude-task","label":"…","state":"running|done","size":123,"started_at":"…","updated_at":"…"}]}` newest first. A job label is its command line; a Claude label is its first output line, or its task ID when empty. Claude start time uses file creation time when available, otherwise modification time. `size`, `offset`, and `next_offset` use the same logical byte coordinate system; this keeps follow cursors monotonic across a trim. Job IDs derive from lease tokens; Claude IDs derive from discovered session and task names, but clients treat both as opaque.

`GET /api/v1/issues/{key}/logs/{id}?offset=N` returns `{"text":"…","offset":N,"next_offset":M,"size":S,"state":"running|done","truncated":false}`. An omitted offset starts at the last 64 KiB. A requested offset before the retained window starts at the window and sets `truncated:true`. The JSON response is at most 64 KiB after redaction and escaping. An unreadable selected file returns HTTP 200 with `{"available":false,"reason":"…"}`. Invalid offsets return 400; unknown keys, IDs, traversal attempts, and peer paths return 404. Both local routes require the current bearer status token even on loopback; configured secrets are redacted from labels and text.

## Failure modes and edge cases

- Missing or cleaned worktree: list jobs, no Claude tasks. Empty sources return an empty `items` array.
- Unreadable selected file: a bounded available-false document. Unreadable or malformed metadata is ignored during discovery; no path from a request is opened directly.
- Concurrent writer, ring wrap, reader and retention: a per-log OS file lock prevents partial reads. Retention uses the current lease snapshot, locks each file before removing it, and exempts running jobs. A crashed writer loses its lease after resource reconciliation, so the log becomes done.
- Claude task state is approximate; a quiet running task may be labeled done until it writes again. Symlinked task output is never opened.
- UTF-8 is decoded lossily at a byte boundary. The byte cursor still advances over the bytes actually read. ANSI stripping affects display only.

## Acceptance criteria

- **AC-1:** A real managed job produces a running log that grows during execution and becomes done when its lease ends, while its returned `Output` remains unchanged.
- **AC-2:** A real Claude Code background Bash `.output` under the worker's slug is listed and followable; a Codex worker has only job logs.
- **AC-3:** Both routes return 401 without the configured bearer token on loopback, and a valid token permits reads.
- **AC-4:** A configured fake secret in a job label and output appears only as `<redacted>` in both routes.
- **AC-5:** Traversal, absolute, foreign-worker and unknown IDs return 404 without reading outside the two allowed sources.
- **AC-6:** A job exceeding 8 MiB keeps the last 8 MiB; stale offsets set `truncated`, and each detail JSON body is at most 64 KiB.
- **AC-7:** Completed job logs older than 24 hours and beyond the 20 newest per key are removed; running logs are retained.
- **AC-8:** The fleet peer path for worker logs returns 404.
- **AC-9:** The Logs tab follows while at bottom, exposes Jump to latest after scroll-up, and stops requests while hidden or closed.
- **AC-10:** The complete local gate and web-only gate pass.

## Acceptance evidence

| AC | Real input and expected observation | Runnable check |
| --- | --- | --- |
| 1 | A managed shell command writes lines before and after a short wait; the listing changes from running to done and the output matches the returned bytes. | `cargo test -p toolu-engine resources::jobs`; `cargo test -p toolu-epic-orchestrator http_logs` |
| 2 | A real Claude Code worker task file in its slug is read; a Codex issue with the same jobs does not enumerate that source. | Real local Claude task probe; `cargo test -p toolu-epic-orchestrator http_logs` |
| 3 | Loopback TCP requests with no, wrong and valid token give 401, 401 and 200. | `cargo test -p toolu-epic-orchestrator http_logs` |
| 4 | A fixture job prints a fake status token in its argv and output; neither response contains it. | `cargo test -p toolu-epic-orchestrator http_logs` |
| 5 | Encoded and literal traversal plus an ID from a second issue yield 404; external sentinel content is absent. | `cargo test -p toolu-epic-orchestrator http_logs` |
| 6 | A managed job emits more than 8 MiB and a stale cursor is requested; the retained suffix, cursor and body bound are checked. | `cargo test -p toolu-engine resources::job_logs`; `cargo test -p toolu-epic-orchestrator http_logs` |
| 7 | Real completed job files with adjusted ages and 21+ runs are pruned; a live lease's file survives. | `cargo test -p toolu-engine resources::job_logs` |
| 8 | Local listener with a configured peer returns 404 for the peer log URL. | `cargo test -p toolu-epic-orchestrator http_logs` |
| 9 | DOM-free follow state transitions and a browser probe show the selected view and visibility behavior. | `node --test crates/epic-orchestrator/assets/dashboard/tests/logs.test.js`; browser probe |
| 10 | All required local quality checks finish with no failures. | `toolu epic job -- cargo xtask gate`; `toolu epic job -- cargo xtask gate --only web` |

## Documentation impact

Document routes, auth, sources, retention and states in `docs/epic-status-api.md`; document the Logs tab and Claude state approximation in `docs/epic-orchestrator/README.md`.

## Open Questions

None. The Claude state approximation and logical-size meaning are explicit v1 contract choices.
