# OpenCode native permissions and gate advice — Brainstorm

**Date:** 2026-10-02  **Issue:** #339 (OP-05)  **Mode:** Delivery, full

## Outcome

Toolu blocks its own denied calls before side effects, leaves OpenCode's configured deny/ask and user choice intact, applies the documented ask fallback, and puts nonblocking gate advice in the model-visible result of the same call.

## Evidence and decision

- The pinned OpenCode 1.18.34 probe records `tool.execute.before` before native permission evaluation. A thrown error stops the call. `permission.ask` was not invoked for a native ask, and a fabricated `additionalContext` property in the before result did not reach the model. Appending to `tool.execute.after.output` did. The after hook does not run for a thrown tool error.
- The current runtime uses the before hook, but drops `advisory` and throws every `ask`. Shared `supportsAsk` still claims OpenCode can ask; `applyDecisionToPermission` and `encodeDecision` still describe a mutable permission effect from the prior adapter.
- Set `supportsAsk("opencode", …)` false. Existing `gateMode` then degrades security guardrail asks to block and judgement asks to advice. A residual ask from a registry module fails closed. Toolu cannot synthesize a proven native prompt on this pin. Native `permission: "ask"` remains OpenCode's own approval path.
- Keep pre-tool advice keyed to the exact session and call ID until a successful `tool.execute.after`, then append it to the existing output and consume it. Clear a reused key before redispatch, and bound pending entries so abandoned calls cannot retain state forever.
- Represent OpenCode's portable decision encoding as callback continuation/refusal, with no “allow” effect that could grant native permission. Keep the exported evaluate-shaped compatibility helper conservative and never change an existing native deny/ask to allow.

## Alternatives

- `permission.ask` as a generated approval prompt: the pinned host did not invoke it for a native ask, and the hook has no proved creation channel.
- A session-level system message for advice: less precise than the after hook and harder to keep isolated across calls.
- Throwing advice as an error: blocks an otherwise permitted tool.

## Risks and checks

Run isolated pinned-host calls with configured native deny and ask, a rejected native ask, a toolu ask, and a registry advisory. Verify files or markers and model-visible tool content. Exercise another plugin's deny and repeated call IDs. Run the full repository gate and existing-host tests before delivery.

## Handoff

Write the spec with explicit fallback behavior and real-host acceptance, then review it before planning.
