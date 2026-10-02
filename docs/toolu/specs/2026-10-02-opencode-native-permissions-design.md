# OpenCode native permissions and gate advisories — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Codex   **Topic:** OP-05, issue #339

## Problem

The former OpenCode permission adapter overwrites native effects with `allow`, and the current native `tool.execute.before` handler drops gate advice. A toolu allow must never grant a call that OpenCode or another plugin rejects. Gate-generated asks also need an honest behavior on the pinned host.

## Non-Goals

1. Do not replace OpenCode's own `permission` configuration or build a synthetic approval UI. The pinned 1.18.34 host does not invoke `permission.ask` for a native ask, and its before hook has no proven prompt channel.
2. Post-tool quality dispatch belongs to #340. This issue uses `tool.execute.after` only to deliver advice produced before the same call.
3. Preserve Claude, Codex, Cursor and Hermes outputs. Do not change core gate policy or add persisted approval state.

## Architecture

Keep `tool.execute.before` as the pre-side-effect gate. It returns normally for allow/advisory; only deny, residual ask, post-block or runtime failure throws. OpenCode then applies its native permission rules and the remaining plugins. `supportsAsk("opencode", event)` is false, so the existing `gateMode` resolves security guardrail ask to block and judgement ask to advice. A registry module that still emits ask fails closed. No toolu decision grants native permission; a native ask requires a fresh OpenCode user choice for each call.

Create one per-plugin-instance advice store, keyed by `sessionID` and `callID`. Before dispatch removes an old entry for that key; advice stores a new entry. The `tool.execute.after` callback consumes the matching entry and appends a bounded, plainly labeled note to `output.output`, the pinned host's model-visible channel. It preserves the tool's original output and any earlier plugin changes. An after callback for another call cannot consume it. Expire old entries and cap the store so native rejection, another plugin's denial or tool errors cannot leave unbounded state. Dispose clears the store.

The legacy evaluate-shaped exported helper remains for compatibility, but `applyDecisionToPermission` never turns an existing native deny or ask into allow. An unsupported generated ask denies unless the event was already a native ask. The shared host encoder models OpenCode as callback continuation/refusal, not a permission effect; `permission/evaluate` is not a native OpenCode event in this contract.

## Interfaces / Schema

- `createToolBeforeHandler(opts, advice?)` retains its existing one-argument callers; optional per-instance advice store receives `sessionID`, `callID`, and advice text.
- `createToolAdviceHooks(opts)` or an equivalent pair returns typed `Hooks["tool.execute.before"]` and `Hooks["tool.execute.after"]` sharing that store. The plugin returns both hooks and clears the store on dispose.
- `supportsAsk("opencode", *) === false`. `nativeEventName("opencode", "permission/evaluate") === null`.
- `encodeDecision("opencode", event, decision)` returns a callback action: continue for allow/advisory and post-tool feedback, throw with reason for pre-tool deny, residual ask or runtime failure. No OpenCode output has `effect: "allow"` or `effect: "ask"`.
- Existing `PermissionEvaluationEvent` stays an adapter compatibility type; `applyDecisionToPermission` leaves native effects unchanged for allow/advisory and never weakens an existing deny.

## Failure modes and edge cases

| Input or state | Observable behavior |
| --- | --- |
| Native config `deny` | Host refuses or removes the tool; toolu cannot re-enable it; no side effect. |
| Native config `ask`, user rejects | Before may run; after does not; target unchanged; retry gets a fresh native decision. |
| Guardrail configured `ask` | Core degrades to a block and before throws; target unchanged. |
| Judgement configured `ask` | Core degrades to advice; tool runs only if native permission also allows; the model sees advice with its result. |
| Registry ask with no gate class | Before throws conservatively; no remembered approval. |
| Another plugin denies after toolu's advisory | No tool result exists; pending advice expires or is removed when that call ID is reused. |
| Same call ID retried, different session/call IDs interleaved | Old advice for a reused key is deleted before redispatch; unrelated entries remain isolated. |
| Tool error or failed native approval | After does not run on the pinned host; bounded expiry prevents unbounded pending state. |
| Non-string or malformed after output | Report a post-tool delivery error to the model without claiming the already executed action was undone. |

## Acceptance criteria

- **AC-1:** In isolated pinned-host runs, toolu allow leaves native `deny` and `ask` effective; rejecting a native ask leaves the target unchanged, and a retry cannot reuse an approval.
- **AC-2:** A guardrail ask blocks before execution, a judgement ask becomes model-visible advice, and a residual registry ask fails closed on OpenCode 1.18.34.
- **AC-3:** A pre-tool advisory reaches the model in the result for exactly the allowed call; a gate deny wins over advice, a denial from another plugin still prevents side effects in either plugin order, and interleaved or retried call IDs do not deliver stale advice.
- **AC-4:** Shared host capability and encoding report no OpenCode dynamic ask or permission grant; the compatibility evaluate helper never weakens a native deny/ask. Existing hosts retain their encoded behavior.
- **AC-5:** The focused real-host smoke, adapter/core tests, host contract check and `bun run test` pass with the pinned SDK and no new warnings.

## Acceptance evidence

| AC | Real input and expected result | Boundary / failure case | Runnable check |
| --- | --- | --- | --- |
| AC-1 | Scripted provider calls real host `bash` or `write` in isolated projects with native `deny` and `ask`; denied/rejected marker absent; an allowed baseline marker exists. | Repeated call after rejection gets a new call ID and remains denied until OpenCode approves. | `bun run smoke:opencode-permissions` |
| AC-2 | Real `.env` write with `protectedFiles: ask` is blocked; a real judgement gate in `ask` mode lets a safe command run and adds its reason to the model-visible tool result. | A direct registry bundle emitting ask still throws before the call. | `bun run smoke:opencode-permissions`; `bun test tools/toolu-opencode/src/adapter/__tests__` |
| AC-3 | A real selected registry advisory for one `bash` call appears in the provider's next tool-result message, while an unrelated call lacks it. A gate deny combined with advice and a second plugin's denial in both loader orders leave markers absent. | Unit tests interleave sessions, reuse one call ID, and check store expiry/cap; thrown tool errors do not reach after. | `bun run smoke:opencode-permissions`; `bun test tools/toolu-opencode/src/plugin/__tests__ tools/toolu-opencode/src/adapter/__tests__` |
| AC-4 | Native effect initialized to deny/ask stays deny/ask under toolu allow/advisory; OpenCode callback encoding has no granting effect and unsupported ask becomes refusal. | Existing Claude/Codex host encoding assertions stay byte compatible. | `bun test packages/toolu-core/src/host/__tests__ tools/toolu-opencode/src/adapter/__tests__`; `bun run test:conformance` |
| AC-5 | Exact pin, checked declarations, full quality gate and live smoke. | A missing host binary or failed host run is a failure, not a skipped pass. | `bun run check:opencode-host`; `bun run smoke:opencode-permissions`; `bun run test` |

## Documentation impact

Update `docs/opencode.md`, `docs/portable-core.md` and the pinned host contract's capability explanation. Keep generated OpenCode resource mirrors synchronized. #363 owns the full migration guide.

## Open Questions

None blocking. The smoke will confirm how the pinned CLI reports native ask rejection and repeated calls; the spec requires observed side effects and model-visible content rather than a particular UI string.

## Spec review

- Acceptance evidence: 🔴 blocker (resolved): the first AC-3 check named another plugin's denial but omitted a combined gate deny/advisory and both plugin orders. AC-3 and its real-host check now require them.
- Requirement alignment: Jev rated the initial coverage near the middle level and scope within this issue. Direct review found and repaired the missing precedence/composition check; feasibility still depends on the pinned-host smoke.
- **Status:** Approved. Every AC has a real-input result, a boundary case and a runnable check; no blocking question remains.
