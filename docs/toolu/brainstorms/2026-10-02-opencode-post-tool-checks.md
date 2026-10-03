# OpenCode post-tool checks — Brainstorm

**Date:** 2026-10-02
**Mode:** Delivery, full

## Capsule

- **Outcome:** A completed OpenCode tool call runs toolu's native post-tool gates and selected registry checks once. Affected edit paths are checked separately, failures reach the model, and quality failures remain visible to later commit and push gates.
- **Material default:** Use the pinned host's `tool.execute.after` and the existing `mapToolCall` and `dispatchPostTool` contracts. Require a verified shell outcome before a post-tool gate may clear state or promote a push waiver. A tool error with no after hook cannot be reported as a completed action.
- **Repository evidence:** OP-01's live probe shows `tool.execute.after` for successful edits and nonzero shell exit, but no after hook for a thrown tool error. The core dispatcher already splits patches into per-path records and runs gate-status, push-waiver, and selected `post-tools.d` modules. OP-05's advice store appends text to the model-visible result.
- **Risk:** OpenCode's result metadata and cancellation shape need a pinned-host capture. Post-tool diagnostics must describe a completed action without implying rollback. A thrown tool error cannot trigger checks through `tool.execute.after`.
- **Handoff:** Write a spec with exact result mapping, missing-outcome behavior, real subprocess checks, and a pinned-host smoke.

## Decisions and trade-offs

1. **Integration:** Translate the host's after event to the existing core PostToolUse payload, using the same validated tool mapping as the before hook. This preserves the native gates and registry selection in one dispatcher. Passing raw host output would omit the core's expected tool names and exit paths; reimplementing checks from file events would duplicate the dispatcher and lose call identity.
2. **State:** The core gate file remains the source of truth. A shell result must have a trustworthy exit status before gate-status can record a pass and push-waiver can promote. A missing or interrupted result must not manufacture success. A nonzero shell exit is still a completed tool result and may record a failing quality command.
3. **Edits:** Dispatch only after the host reports completion. The core patch walker handles additions, updates, deletions, and both sides of moves. Post-tool module diagnostics are appended to the host result; a post block reports failure after execution, not a claim that the edit was undone.
4. **Scope:** OP-06 wires the core bridge and proves representative quality behavior. Leaf quality module changes belong to OP-18 through OP-20. Other host integrations retain their existing payloads and behavior.

Jev compared reuse of the normalized after event and core dispatcher with raw after payloads and file-event reimplementation; it preferred the reuse path (choice A, confidence 0.98). The decision also follows the observed host contract and the issue's acceptance criteria.
