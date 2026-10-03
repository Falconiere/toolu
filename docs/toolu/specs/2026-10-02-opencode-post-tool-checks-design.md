# OpenCode post-tool checks — Design

**Date:** 2026-10-02 **Status:** Approved **Author:** Codex **Topic:** OP-06 native after-hook dispatch

## Problem

OpenCode currently publishes selected `post-tools.d` modules but never dispatches them. Completed edits therefore miss language quality checks; completed shell quality commands and pushes miss the native gate-status and push-waiver updates. The model cannot see post-tool failures, and a later commit or push may proceed without the quality failure that another host would record.

## Non-Goals

1. Rework the core dispatcher or the leaf language rules owned by OP-18 through OP-20.
2. Claim that a post-tool block undoes a completed edit, command, or push.
3. Synthesize an after event for a thrown tool error or native permission rejection. The pinned host does not emit one; it sends its own error to the model.

## Architecture

Add one OpenCode `tool.execute.after` bridge beside the before adapter. Reuse `mapToolCall` to translate the host's final arguments into a core request and `dispatchPostTool` to run native `gate-status` and `push-waiver` followed by selected registry modules. The bridge owns a bounded per-instance `(sessionID, callID)` completion guard, reset by the before hook and cleared on dispose. It composes with OP-05's pending advice so both messages reach the same result without dropping either. A call is dispatched once; the core dispatcher alone walks each normalized edit record.

The core quality gate file remains the source of truth. For shell results, map the pinned host's numeric `output.metadata.exit` to `tool_response.metadata.exit_code`; also supply the host output text and interruption flag in the core payload. A nonzero exit is a completed failure and runs the post gates. A missing, invalid, or interrupted shell outcome cannot prove success: skip state-changing post dispatch and append a diagnostic. Completed edit, write, and patch results dispatch after the host has made their filesystem changes. Other recognized tools may dispatch for selected registry checks.

The bridge interprets a core post block or advisory as model-visible text appended to `output.output`, labeled as a check after execution. It must retain the original result and any pre-tool advice. The existing quality modules record failing per-file state; gate-status records a failed quality command. Those states are read by the existing pre-tool quality gate on later commit and push calls.

## Interfaces / Schema

- `createToolPostHandler(options, advice)` returns a `tool.execute.after` handler plus `begin(call)` and `clear()` for call isolation. `options` uses the same repo root, data root, user config root, project cwd, host environment, and selected plugin specs as `createGateDecider`.
- The core JSON request carries `session_id`, `tool_use_id`, `cwd`, `tool_name`, and `tool_input` from `mapToolCall`. For bash it adds `tool_response.metadata.exit_code: <integer>` and `tool_response.interrupted: false`, plus `tool_output: <host output>`. Non-shell responses carry the host output and metadata without inventing an exit status.
- Post-result interpretation accepts empty stdout as allow, `decision: "block"` with `reason` as failure, and `hookSpecificOutput.additionalContext` / `systemMessage` as advisory. A dispatcher exit 2, other nonzero exit, or malformed nonempty JSON becomes an explicit post-check failure message. Neither a block nor a dispatch failure is described as a rollback.
- The existing `tool.execute.after` hook in `plugin/hooks.ts` calls the bridge and OP-05's advice delivery in a deterministic order. A duplicate after call for the same tool call cannot re-run the dispatcher or duplicate a diagnostic.

## Failure modes and edge cases

- A thrown or denied tool has no after hook. No post module runs or records a pass; the host's tool error remains model-visible. An old gate status is left intact.
- An after event with an unknown tool is ignored by core dispatch but can still deliver matching pre-tool advice. A malformed recognized tool payload becomes a visible post-check failure and cannot record success.
- A shell result with no finite integer exit, or an interruption/cancellation marker, cannot clear a failing quality state or promote a push waiver. If after fires, the model sees that post checks lacked a confirmed outcome.
- The core patch parser must see the complete final patch. A malformed patch is a post-check failure rather than a partial per-file pass. A valid move checks source and destination; a delete clears the removed path's quality entry.
- Registry module failure, malformed dispatcher output, or a thrown bridge error cannot silently replace the tool's original result with a success claim. The model sees the post-check failure. A quality module's own gate entry persists until its normal recovery path clears it.
- Concurrent sessions use `(sessionID, callID)` identity. Duplicate after callbacks and reused IDs do not run old checks or deliver old advice. The completion guard has a finite cap and expiry and is cleared on disposal.

## Acceptance criteria

- **AC-1:** A completed OpenCode edit/write and a multi-file patch run the selected post-edit module once per affected path, including delete and both sides of a move; a post violation appears in the next model-visible tool result.
- **AC-2:** A completed shell quality command with exit 0 clears its tracked failure, while a nonzero exit records failure and causes the next commit and push to be refused by the existing gate.
- **AC-3:** A failed, interrupted, or outcome-unknown tool cannot record a passing quality status or promote a push waiver; the host error or bridge diagnostic reaches the model where the host has a result channel.
- **AC-4:** A post block, advisory, or runtime dispatch failure preserves the original tool output and reports that the tool already executed; pre-tool advice and post-tool diagnostics both appear once for their matching call.
- **AC-5:** The pinned OpenCode host actually loads the adapter and delivers the result, state, and later commit/push behavior in an isolated project. Existing Claude and Codex conformance and the repository gate pass.

## Acceptance evidence

| AC   | Real input and expected result                                                                                                                                                                                 | Boundary and runnable check                                                                                   |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| AC-1 | Temp TypeScript project with selected ts-quality registry bundle; real edit/write and a patch that adds, updates, deletes, and moves files. Count module invocations and inspect gate entries and result text. | Duplicate after and malformed patch; `bun test tools/toolu-opencode/src/adapter/__tests__/tool-post.test.ts`. |
| AC-2 | Temp git project and real native gates: `bun run test` exits 3 then 0. Inspect gate file and run the real pre-tool commit/push decider.                                                                        | Nonzero exit with following shell command; same adapter test and pinned-host smoke.                           |
| AC-3 | Pinned host rejected read/edit and bash exit 3; direct bridge case for missing/invalid/interrupted metadata. Confirm no false pass or waiver.                                                                  | `bun run smoke:opencode-posttool` and adapter test.                                                           |
| AC-4 | Real registry module emitting a post block/advisory, plus a failed module; original output contains exactly one labeled diagnostic and matching pre advice.                                                    | Repeated and cross-session call IDs; adapter and plugin hook tests.                                           |
| AC-5 | Isolated pinned `opencode-ai@1.18.34` session with scripted loopback provider and selected catalog; observe file bytes, model tool messages, gate file and denied later commit/push.                           | Full `bun run test`, OpenCode smoke, existing conformance checks.                                             |

## Documentation impact

Update `docs/opencode.md` and `docs/opencode-host-contract.md` with post-tool coverage, result semantics, and the no-after-on-throw limitation. Update the lifecycle support table's tool/post entry and its test.

## Open Questions

None blocking. The pinned host smoke will pin the exact interruption metadata. A missing or invalid exit takes the conservative unknown-outcome path.

## Spec review

Status: Approved. The three issue criteria have representative real inputs and observable checks; the no-after case and the meaning of post-tool diagnostics are explicit. Jev rated the stated issue-to-evidence alignment 0.74. Review corrected the claim about unknown interruption markers; the spec now requires a validated exit and checks interruption metadata observed on the pinned host.

## Decision evidence

OP-01 probe evidence shows `tool.execute.after` for completed edits and bash exit 3, and no after callback for a thrown tool error. The pinned SDK declares the after input's args and output's title, output, and metadata. Jev preferred reuse of normalized after dispatch over raw payloads or file events (choice A, confidence 0.98), and preserving state when no after exists over preemptive failure or inferred completion (choice A, confidence 1).
