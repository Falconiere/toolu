# OpenCode core pre-tool enforcement — Brainstorm

**Date:** 2026-10-02  **Mode:** Delivery, full

## Capsule

- **Outcome:** On pinned OpenCode 1.18.34, selected core and registry pre-tool rules refuse protected operations before side effects, including MCP and task policies.
- **Boundary:** Keep the existing `tool.execute.before` entry and `@toolu/core` dispatch behavior. Permission-prompt handling and post-tool feedback belong to #339 and #340.
- **Evidence:** #336 established that a `tool.execute.before` throw stops a real host call, #337 captured and normalized native calls, and `dispatchPreTool` already walks the nine core modules and registry in order. The standalone `mcpHook` and `agentTierHook` carry their existing host policies. OpenCode's current registry presence is `unknown`, so stale disabled modules need an explicit selected-spec filter.
- **Risk:** The task tool may omit a model tier, and the host has no plugin ask channel. Keep the existing fail-closed ask behavior; test a supplied task model against an actual ledger and verify that an absent model does not invent a mismatch.
- **Handoff:** Specify the dispatch routes, selection filter, failure behavior, and real-host evidence.

## Material axes and choices

| Axis | Choice and evidence | Risk or boundary |
| --- | --- | --- |
| Enforcement | Reuse `dispatchPreTool` for ordinary normalized calls. It already preserves built-in/registry order, per-path patch walks, and deny over ask over advisory. | A parallel OpenCode runner would duplicate precedence and drift from Claude/Codex. |
| MCP and task | Route MCP calls through the core `mcpHook`; run task calls through ordinary dispatch and the core `agentTierHook`, combining their decisions with deny priority. | The standalone MCP hook intentionally skips general registry modules, matching existing hosts. |
| Selection state | Pass the resolved enabled plugin specs from the OpenCode bootstrap into registry dispatch as an optional filter. Leave the existing-host default unchanged. | Stale registry files can persist; the filter must apply at every call. |
| Failure | Reject malformed gated calls, dispatch failures, and denied/asked decisions by throwing in `tool.execute.before`. | The pinned host fails open on plugin init throw, so retain #336's deny-all readiness hook. |
| Verification | Use real temp repos and committed bundles for focused tests, then a scripted provider driving the pinned actual host for side-effect checks. | The live harness remains opt-in until #362 makes it a CI gate. |

## Alternatives considered

1. A separate OpenCode gate runner would require a second implementation of ordering, registry behavior, and patch walks. It offers no needed capability and expands the regression surface.
2. Relying on bootstrap alone to remove disabled modules leaves stale registry files executable because OpenCode has no installed-plugin snapshot in the shared registry gate.

Jev assessed the existing-dispatch route as the strongest fit given the issue requirements and repository evidence (choice A, confidence 0.97). Feasibility and exact behavior remain subject to tests.
