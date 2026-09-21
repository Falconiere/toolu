# Portable Bun/TS core contracts

**Issue:** [#205](https://github.com/Falconiere/toolu/issues/205) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Status:** Contract freeze for implementation in #208/#210/#211/#204. This document does **not** claim complete OpenCode support.

## Pins

| Component | Pin | Discovery |
|-----------|-----|-----------|
| OpenCode CLI | `v2.0.12` | `$OPENCODE_BIN` or `command -v opencode`; `opencode --version` |
| Plugin SDK | `@opencode/plugin@2.0.12` | `npm view @opencode/plugin version` (must match CLI) |
| Bun | `1.4.2` (workspace requires 1.4.x) | `bun --version` |
| Docs baseline | OpenCode V2 plugin hooks | https://opencode.ai/v2/docs/build/plugins |
| Unsupported | `opencode-ai@1.18.31` (V1 line) | Do not target for this port |

Platforms for #212: macOS and Linux with Bash ≥5, `jq`, Bun 1.4.x. Windows out of scope until probed.

## Package boundaries

| Path | Owns | Must not own |
|------|------|--------------|
| `packages/toolu-core/` | Zod schemas, normalization, decisions, policy, bash-bridge protocol | `@opencode/plugin`, host SDKs, generated skills |
| `tools/toolu-opencode/` | OpenCode `setup`, hooks, generators, host config | Duplicated Zod contracts |
| Conformance CLI (default `tools/toolu-conformance/`, name finalized in #208) | Second real consumer of core exports | Host SDK |

Root: one `bun.lock`, Bun-only scripts/tests, frozen installs. Unified release `vX.Y.Z` across root, packages, and every `plugin.json`. Claude Code/Codex keep native Bash paths and gain **no** mandatory Bun dependency.

### Core export map (frozen for #210)

| Export | Responsibility |
|--------|----------------|
| `./decision` | Discriminated decision union |
| `./events` | Normalized event schemas + parsers |
| `./bridge` | Bash bridge request/response schemas |
| `./policy` | Classification enum + precedence helpers |
| `./config` | `toolu.config.json` Zod (`version: 1`) |

## Zod boundary rules

| Envelope | Unknown fields | Missing required | Bad version |
|----------|----------------|------------------|-------------|
| Persisted gate/state JSON | reject (`strict`) | `runtime_failure` | fail closed |
| `toolu.config.json` | reject unknown top-level keys | optional keys may default | unsupported major → fail closed |
| Host event envelope | passthrough extras; re-encode known fields only | fail closed for decide fields | N/A |
| Bash bridge stdout | reject non-object / unknown discriminant | `runtime_failure` | N/A |
| Bridge stderr | free text | ignored | N/A |

Derive types with `z.infer`. No `any`, unchecked casts, or non-null escapes to bypass validation.

## Decision contract

| Outcome | Effect | Mapping from Bash today |
|---------|--------|-------------------------|
| `allow` | proceed | silent success |
| `ask` | host prompt | `permissionDecision: "ask"` (`plugins/toolu/hooks/lib/gate-mode.sh`) |
| `deny` | hard block | `permissionDecision: "deny"` |
| `advisory` | context only | `additionalContext` / `systemMessage` |
| `post_block` | post-tool feedback | PostToolUse `decision: "block"` (`plugins/toolu/hooks/lib/dispatch.sh`) |
| `runtime_failure` | fail closed | parse/bridge/timeout |

**Ask degradation** (`plugins/toolu/hooks/lib/host.sh` `toolu_supports_ask`): Codex cannot prompt — judgement gates `ask→advise`, security guardrails `ask→block` (`gate-mode.sh`, `plugins/toolu/hooks/docs/gates.md`). OpenCode supports permission `ask`; hosts without prompt use the same class rules.

**Precedence:** deny beats ask beats advisory merge; multi-file patches hold ask while walking and emit the first deny immediately (`dispatch.sh`).

## Event vocabulary

Normalized: `session/{start,resume,clear,unload}`, `prompt`, `pre_compact`/`compaction`, `permission/evaluate`, `tool/{pre,post}`, `shell/pre`, with `sessionId`, `toolCallId`, `cwd`, `projectRoot`, `worktree`, multi-file edit records, cancellation, state scope.

## Bash bridge protocol

`protocolVersion`: **1**

Request (stdin JSON to assembled dispatcher) and response (stdout JSON) shapes match the design spec. Truncated/non-JSON stdout → `runtime_failure` (enabled pre-tool → deny). Incomplete assembled registry → bootstrap/`runtime_failure`, never silent allow. Invoke assembled registry modules and built-in dispatchers — not raw concern fragments. Exit 0 does not prove registry readiness. Not every plugin has `register.sh`.

## Policy split (shared with #209)

Classification vocabulary (only these tokens):

- `shell-out`
- `port-native`
- `port-new`
- `no-map`

Implementation choice is orthogonal to host interception. Required unsupported enforcement is a **release blocker**, never `no-map`.

Exhaustive per-source rows: [docs/gate-coverage-matrix.md](gate-coverage-matrix.md) (checked by `bun run tooling/gate-coverage-inventory.ts check`).

## Protected-files gate trace

**Fixture:** `tooling/fixtures/portable-core/protected-files-pre.json` (Edit targeting `/repo/.env`).

1. Host carries an edit (or Bash write) to a protected path.
2. Normalize via edit records (`plugins/toolu/hooks/lib/edit-records.sh`).
3. `plugins/toolu/hooks/pre-tools/modules/protected-files.sh` + `toolu_gate_mode protectedFiles` (`gate-mode.sh`).
4. Mode `block` → decision `deny`. OpenCode adapter **must** map that to `permission.effect = "deny"` (or `permission.rules`) **before** the write. Prompt text alone is not enforcement.
5. Post-tool cannot un-write a completed edit.

## OpenCode interception (capability table)

| Mechanism | Hard-block? | Role for toolu |
|-----------|-------------|----------------|
| `ctx.permission.hook("evaluate")` / `ctx.permission.rules` | **Yes** | Primary deny/ask path |
| `ctx.tool.hook("execute.before")` | No (mutate/observe) | Input inspection only unless a future probe proves otherwise |
| `ctx.shell.hook("create.before")` | No (mutate only) | Env/cwd/timeout mutation; deny via permission |
| `ctx.session.hook("prompt")` | No typed rejection | Must not be used as deny |

Source: https://opencode.ai/v2/docs/build/plugins (V2).

### Capability-results

<!-- portable-core-capability-results:start -->
```json
{
  "recordedAt": "2026-09-21",
  "cliVersion": "v2.0.12",
  "sdkPackage": "@opencode/plugin",
  "sdkVersion": "2.0.12",
  "bunVersion": "1.4.2",
  "docsUrl": "https://opencode.ai/v2/docs/build/plugins",
  "permissionEvaluateHardDeny": true,
  "permissionRulesHardDeny": true,
  "toolExecuteBeforeHardDeny": false,
  "toolExecuteBeforeMutateOnly": true,
  "shellCreateBeforeHardDeny": false,
  "shellCreateBeforeMutateOnly": true,
  "sessionPromptTypedRejection": false,
  "notes": [
    "permission.hook(evaluate) may set effect to deny; configured deny skips the hook",
    "tool.execute.before and shell.create.before document mutation/observe only — insufficient alone for hard deny",
    "session.prompt has no typed rejection API"
  ]
}
```
<!-- portable-core-capability-results:end -->

Refresh with `bun run tooling/opencode-capability-probe.ts` (live CLI) or verify with `PORTABLE_CORE_PROBE_MODE=fixture` (CI).

## Release blockers (parity)

Until a live probe proves otherwise for a required action class:

1. Any required gate that cannot be mapped through `permission.evaluate` / `permission.rules` on the pinned CLI.
2. Treating `tool.execute.before`, `shell.create.before`, or prompt rewriting as a substitute for hard deny.
3. Advertising OpenCode enforcement when bootstrap/CLI pin check fails.

## Related docs

- Runtime gate modes: [plugins/toolu/hooks/docs/gates.md](../plugins/toolu/hooks/docs/gates.md)
- Config schema: [docs/config.md](config.md)
- Tracking: [#205](https://github.com/Falconiere/toolu/issues/205) / epic [#203](https://github.com/Falconiere/toolu/issues/203)
