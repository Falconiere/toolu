# Worker screen endpoint live probe (#654)

**Date:** 2026-10-27. Herdr 0.9.5, protocol 22. The isolated named session `toolu-654-screen-live` ran headless with `headless_cols = 800` and `headless_rows = 100`. A temporary resource root held a paused epic with `probe-claude` and `probe-codex` mapped to real Herdr agents. `target/debug/toolu epic engine --http 127.0.0.1:0` served the route with a disposable token. The engine and session were stopped and the session deleted after the probe; no default-session pane was touched.

## Real hosts and token boundary

`herdr agent start probe-claude --kind claude --pane w1:p1` started Claude Code 2.1.311; `herdr agent start probe-codex --kind codex --pane w2:p1` started Codex 0.164.0. Both received a read-only prompt containing `SCREEN_PROBE_CANARY_654` and the configured fake token. Authenticated `GET /api/v1/issues/{key}/screen` returned 200 with the current visible pane for both; Claude showed its reply, and Codex was sampled while `agent_status:working`. The source code and real-socket test assert `agent.read` with `source:visible`, `format:text`, and no `lines` parameter. No `recent` read is used by this route. AC-1: pass.

Missing and wrong bearer tokens both returned 401 on loopback. The real-socket test also verifies 403 `control_disabled` without a configured token and no Herdr call after either refusal. AC-2: pass.

The canary appeared in both responses; the fake token appeared as `<redacted>` in Claude's prompt and reply, and neither response contained its raw value. The real-socket test checks a cache hit after the configured secrets change. AC-3: pass.

## Oversized real pane

Claude Code's `!` shell command printed a large fixture; its own UI collapsed the output. Suspending that same real Claude process with Ctrl+Z kept the Herdr agent binding, and `herdr pane run w1:p1` printed 99 deterministic 760-character lines and a final `SCREEN_TAIL_654_😀` line in its terminal. `herdr agent read probe-claude --source visible --format text` returned 73,853 bytes. The authenticated screen response was 65,524 bytes of valid JSON, included the final Unicode marker, and reported `lines:89`, equal to `text.lines()` after truncation. The initial lines were removed. The real-socket test checks the JSON byte boundary and UTF-8 tail too. AC-4: pass.

Two immediate authenticated reads returned the same `sampled_at` and text; one after 1.1 seconds got a new timestamp. The real-socket test counts one same-key `agent.read`, forces nine overlapping stalled calls to get `503 herdr_busy`, and checks `/overview` stays responsive. AC-8: pass.

## Missing and unavailable agents

Closing the isolated Codex pane made its screen route return `200 {"state":"not_running"}`. Stopping only `toolu-654-screen-live` made the Claude route return `200 {"available":false,"reason":"not running"}` while `/api/v1/overview` still returned 200. The real-listener test covers `TOOLU_EPIC_HERDR=off`, scripted mode, protocol mismatch and absent agent/pane. AC-5: pass.

The real-socket listener test presents a blocked issue and visible dialog, and checks the session-aware `herdr --session ... agent attach ...` text. A real host trust dialog was not reproduced in this isolated run; the live agents were idle or working. AC-6: pass for the endpoint contract; real trust-dialog display remains unobserved.

The configured peer HTTP test verifies that `/api/v1/fleet/peers/{name}/issues/{key}/screen` is 404 and does not call the peer. AC-7: pass.

AC-9: the pre-review snapshot passed the complete `cargo xtask gate` in the isolated Linux `deploy-gate` runner (receipt `f32e9dc1c93dadba0937febd0f063713e5e052f5`). The final rebased branch gate receipt is recorded in PR #693 before merge.
