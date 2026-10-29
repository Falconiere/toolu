# Cancel timed-out herdr connects — Design

**Date:** 2026-10-29   **Status:** Approved   **Author:** Codex (epic worker, Falconiere/toolu#747)   **Topic:** Bound Unix socket connection resources for live HTTP requests

## Problem

`http_live::connect` returns after one second when herdr's Unix listen backlog is full, but its blocking connector thread can remain alive indefinitely. Repeated requests can retain threads and descriptors after their HTTP deadlines.

## Non-Goals

1. No change to the control socket's idle request sessions (#748).
2. No change to herdr protocol, HTTP response schema, cache, or the eight-call limiter.
3. No timeout on a long-running engine `wait` or `replace` reply.

## Architecture

Use the existing `nix` dependency's safe Unix socket API. Create a nonblocking, close-on-exec Unix stream descriptor and retry an in-progress connect with a short bounded pause until the one-second deadline. Linux returns `EAGAIN` for a full Unix backlog; polling that descriptor can report writable with no socket error while it remains unconnected, so a retry is required. Drop the descriptor on timeout or failure. Convert a connected descriptor to blocking `UnixStream` with the existing one-second read/write timeouts. This removes the detached connector thread and makes cleanup immediate on return. Keep `Limiter` and callers unchanged.

## Interfaces / Schema

`http_live::connect(&Path) -> Result<UnixStream, String>` and its `not running` error mapping remain unchanged. `Cargo.toml` enables the needed features on the workspace's existing `nix` dependency. No public API or JSON changes.

## Failure modes and edge cases

- Missing/refused socket, failed connect, and deadline expiry return `not running`; the owned descriptor closes on every error. Interrupted connects retry until the deadline.
- A connection that succeeds at the deadline is accepted only if the deadline has not passed. Otherwise it closes without a ping or request write.
- Success restores blocking I/O and the existing per-operation read/write timeouts. Normal ping and `agent.list` use separate connections as before.
- Saturation still uses eight limiter slots and returns `503 herdr_busy` for an excess uncached request. Slots are released when a request finishes.
- Linux and macOS use the same safe nonblocking connect loop; platform-specific backlog behavior is measured with real listeners.

## Acceptance criteria

- **AC-1:** A real Unix listener with a full backlog causes each live connect to return no later than its one-second deadline, with no connector threads or descriptors retained after return across repeated attempts.
- **AC-2:** On a system where backlog saturation leaves a connect pending, if the listener accepts it after its deadline, that connection closes without receiving ping or request bytes.
- **AC-3:** A real herdr protocol-22 socket still serves a normal live sample; missing socket, disconnect, and stalled read return the existing unavailable response, with one-second read/write deadlines.
- **AC-4:** With eight live call slots held, an excess uncached request returns `503 herdr_busy`; released slots can be reused.
- **AC-5:** The full repository gate passes on the final pushed head; two independent medium code reviews approve that head with full coverage and nonempty JSON reports.

## Acceptance evidence

| AC | Real input | Expected observation | Boundary | Runnable check |
| --- | --- | --- | --- | --- |
| AC-1 | Bound Unix listener with backlog deliberately filled by real connections | Repeated connect calls settle by one second; process thread/FD counts return to baseline | macOS can refuse immediately; Linux may leave a connect pending | Focused `http_live` test and before/after release measurement on Linux and macOS |
| AC-2 | Same listener, then accept just after timeout on a system with a pending connect | Accepted late peer receives EOF and zero request bytes | Deadline crossing; macOS immediate refusal has no late peer | Focused `http_live` test on Linux and macOS |
| AC-3 | `HerdrFixture`, absent socket, and real silent/disconnecting Unix listeners | Existing success and unavailable JSON; stalled reads end after the read timeout | Disconnect and shutdown | `cargo test -p toolu-epic-orchestrator http_live` |
| AC-4 | Real HTTP listener plus held limiter permits | Ninth request is 503; later request can acquire a slot | Saturated cap | Existing and extended `http_live` tests |
| AC-5 | Final pushed branch head | Green gate; two independent approvals with zero unreviewed files | All changed files | `cargo xtask gate`; two `toolu code-review run --base origin/main` calls |

## Documentation impact

Update `docs/epic-status-api.md` to state that a timed-out herdr connect closes its descriptor before returning, including the bounded cleanup behavior.

## Open Questions

None. The issue supplies the deadline, cap, and supported operating-system requirement.

## Spec review (2026-10-29)

No blocking or should-fix findings. The criteria cover the deadline, cleanup,
late connection, normal and error responses, limiter, and local delivery gates
with real socket inputs and runnable checks.

The macOS backlog probe returned `ECONNREFUSED` immediately, so AC-1 uses a
one-second maximum and AC-2 applies when the kernel leaves a connect pending.
The Linux probe returned `EAGAIN` for a full backlog, and polling that socket
reported writable plus hangup without completing the connection. The approved
implementation uses bounded retries rather than treating poll readiness as success.

**Status:** Approved
