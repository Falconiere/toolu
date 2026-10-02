---
name: verify-opencode-permissions
description: Verify OpenCode native permissions, toolu gate fallbacks, and advice on the pinned host.
metadata:
  toolu:
    origin: agent
    created: 2026-10-02T16:26:06Z
---
## When to Use

Use when changing toolu's OpenCode pre-tool decisions, native permission behavior, or model-visible advice on the pinned host.

## Procedure

1. Read `docs/opencode-host-contract.md` and the pin in `tools/toolu-opencode/contract/pin.json`. Check relevant `tool.execute.before`, `tool.execute.after`, and permission behavior in the probe evidence before changing the adapter.
2. Keep `opencode.json` permission decisions authoritative. A toolu allow returns normally; a deny throws before effects. Degrade a gate ask by class: guardrails block and judgement gates advise. Deliver advice only through the matching successful `tool.execute.after` result.
3. Run focused adapter, plugin, host encoding, and gate-mode tests. Then run `bun run smoke:opencode-permissions` against the pinned CLI. Inspect the scripted provider's tool-result content and marker files, including repeated native ask rejection and a second plugin's denial in both load orders.
4. Run `bun run smoke:opencode-pretool`, `bun run smoke:opencode-entry`, `bun run check:opencode-host`, and the repository's required gate. Update `docs/opencode.md`, `docs/portable-core.md`, and `docs/opencode-host-contract.md` when behavior changes; regenerate the committed OpenCode resource mirror.

## Pitfalls

- The declared `permission.ask` plugin hook is not invoked on the pin; OpenCode's own permission prompt still operates after the before hook.
- Mutating before-hook output does not deliver advisory text to the model. The after hook does not run when the tool throws.
- When testing plugin load order, create local plugin files in the intended order; names alone do not establish hook order in the isolated profile.
- Keep per-call advice bounded and keyed by session and call ID. A retry or denied call must not inherit a previous advisory or approval.

## Verification

Require the real host to leave denied target bytes unchanged, to issue a fresh native permission ID for each rejected attempt, and to include advice only in the matching successful tool result. Confirm unrelated plugin denials still stop execution. Focused tests, pinned-host smokes, host contract check, generated-surface check, and the full repository gate must pass.
