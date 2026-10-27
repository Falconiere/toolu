# Worker screen endpoint — brainstorm

**Outcome:** A token-gated local GET returns the visible worker pane for a working, idle or blocked agent, refreshed within one second, with safe unavailable states.

**Boundary:** Read one worker's herdr `agent.read` over the socket with `source: visible`, `format: text` and no `lines` parameter. Never use `recent`, the engine client with CLI fallback, or the peer proxy. No web input to the pane.

**Repository evidence:** #651 provides `http_live::Limiter`, a protocol-22 socket client and `World` agent status. #650 provides `http_access::authorized` and live secret reload. `http_read::finite_response` owns local GET routing. Herdr 0.9.5's API schema requires `target` and `source` for `agent.read`, accepts `format`, and returns `read.text`; its CLI confirms `visible` and `text`. The existing `http_live.rs` is near the Rust file limit.

**Approach:** Add a focused `http_screen.rs` with a per-key, one-second raw read cache; reuse the limiter and socket helpers from #651. Map the issue key to `issue.agent`, obtain status from `World`, and redact with the *current* secrets when forming the HTTP response. Redact before trimming to 64 KiB so the tail cannot expose a fragment of a configured secret. Keep the bottom of valid UTF-8 text. Report `lines` as the number of displayed text lines.

**Alternatives:** Extending `http_live.rs` would cross its file-size limit and couple distinct cache keys. A generic herdr service adds an abstraction without another user. Calling the existing engine `Client` would spawn the CLI on protocol mismatch and journal every poll.

**Failure behavior:** Missing agent or pane is `not_running`; a down/off/scripted/protocol-mismatched herdr is `available: false` with a short reason; a full limiter is `503 herdr_busy`. Unknown issue keys remain `404`. A blocked agent gets the session-aware attach command, with no new control operation.

**Risk:** Herdr's visible screen may contain secrets outside configured redaction; this is why the token is mandatory even on loopback. A cache must not retain a redacted response across token rotation or secret changes. Socket transport and test-fixture response shape must be checked against the installed herdr before implementation.

**Handoff:** Specify observable response shapes and real-pane acceptance checks, then review the spec before planning.
