# Portable Bun/TS core contracts

**Issue:** [#205](https://github.com/Falconiere/toolu/issues/205) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Status:** `@toolu/core` provides Zod contracts, native gates and dispatchers. Fixture-suite conformance evidence lives in `@toolu/conformance`; see [`conformance-report.md`](conformance-report.md).

## Pins

| Component | Pin | Discovery |
|-----------|-----|-----------|
| Bun | `1.4.2` (workspace requires 1.4.x) | `bun --version` |

Runtime contract: [`runtime.md`](runtime.md). Bash and jq are not prerequisites. Windows is out of scope until probed.

## Package boundaries

| Path | Owns | Must not own |
|------|------|--------------|
| `packages/toolu-core/` | Zod schemas, normalization, decisions, policy and dispatch | Host SDKs, generated skills |
| Conformance CLI (`tools/toolu-conformance/`) | Second real consumer of core exports | Host SDK |

Root: one `bun.lock`, Bun-only scripts/tests, frozen installs. Unified release `vX.Y.Z` across root, packages, and every `plugin.json`. Bun 1.4.x is the documented prerequisite for every host, including Claude Code and Codex (epic [#247](https://github.com/Falconiere/toolu/issues/247) retires the #203 no-Bun constraint); see [`runtime.md`](runtime.md).

TS quality foundation (oxlint/oxfmt, strict `tsc`, structural guardrails, knip, jscpd, Zod-only validators) is adopted in [#213](https://github.com/Falconiere/toolu/issues/213) — see [`docs/conventions-adoption.md`](conventions-adoption.md). Bun workspaces (`packages/toolu-core`, `tools/toolu-conformance`) and the mandatory CI `typescript` job land in [#208](https://github.com/Falconiere/toolu/issues/208): `bun run test:ts` must pass before runtime implementation merges. Missing Bun/lockfile/tooling fails closed (no successful skip).

### Core export map

| Export | Responsibility |
|--------|----------------|
| `./process` | Bounded `runCommand` with concurrent output draining, deadlines, cancellation, native exit status and owned process-group cleanup |
| `./resources` | Shared agent/job admission, durable leases, atomic locks, pressure hysteresis, host cooldowns and fenced ownership transitions |
| `./resources/binding` | Git-worktree resource binding used without inherited environment variables |
| `./resources/jobs` | `runManagedJob` admission and lease ownership through process-group completion; bound ledger checks use this path |
| `./decision` | Discriminated decision union |
| `./events` | Normalized event schemas + parsers |
| `./policy` | Classification enum + precedence helpers |
| `./config` | `toolu.config.json` Zod (`version: 1`); loader and resolvers ported from the bash config libs: `loadConfig` (jq `*` merge, fail-closed envelope), `enabled`/`model`/`codexModel`/`configString`, `qualityThreshold`, `docsSync*`, `gateMode`/`gateDecision`, `permissionsAutowrite`, and typed `settings/*` loaders ([#253](https://github.com/Falconiere/toolu/issues/253)) |
| `./state` | Persisted state ported from the bash libs, byte-compatible for v1: `recordGateFailure`/`clearGateFile`/`readGateFile` (multi-slot gate file, atomic temp-and-rename writes under `<gate>.lock`, strict `GateFileSchema` with implicit version 1), `sweepState` (same TTL, merge and retention rules), `diffSha`, `telemetryAppend` (closed per-event `TELEMETRY_EXTRAS`, so no free-form payload is ever logged) and `normalizeEditRecords` (Edit, Write, MultiEdit, `apply_patch`) ([#255](https://github.com/Falconiere/toolu/issues/255)) |
| `./registry` | Cross-plugin hook modules as bundled ESM (`<config>/toolu/<dir>.d/<spec>__<name>.js`): `RegistryModule` contract, `runRegistry` (byte-order walk, installed-plugin gating, optional host-selected spec filter, per-module error isolation, stop after deny/block), `registerModules`/`runRegisterHook` (atomic sync, same-prefix prune) and `pruneInactiveModules` (Codex snapshot); parity with `registry.sh`, `dispatch.sh` and the plugins' `register.sh` ([#257](https://github.com/Falconiere/toolu/issues/257), [docs/registry.md](registry.md)) |
| `./ledger` | The delivery-flow plan ledger, the push verdict and push-review waivers. For v1 files they retain parity with the historical Bash implementation. `ledgerMain`/`ledgerRun`/`ledgerStatus`/`ledgerPreflight` (bundled CLI `plugins/toolu/hooks/dist/plan-ledger.js`; each step check runs in its own process group under a native `PLAN_LEDGER_STEP_TIMEOUT`), `parseSteps`/`docField`/`parseAcs`/`checkAcRefs`, `verdictMain`/`verdictReport` (bundled CLI `plugins/toolu/hooks/dist/verdict.js`; quality, plan, review v2, docs), and `pushWaiverPend`/`pushWaiverPromote`/`pushWaiverMatches` ([#256](https://github.com/Falconiere/toolu/issues/256)). Skills run the Rust port, `toolu ledger` ([#421](https://github.com/Falconiere/toolu/issues/421)); the bundled CLIs stay as its reference behind the `TOOLU_IMPL` seam |
| `./startup` | What the leaf plugins' SessionStart hooks share: `publishWrapper` (symlink a plugin file at `<config root>/<dir>/<name>`, never over a user's file), `bunOnPath`/`bunAdvisory`, `sessionContext` (bounded to `MAX_CONTEXT_CHARS`) and `renderHookOutput`, and the Codex dependency check `codexMissingPlugins`/`codexDependencyNotice` with host-native install commands. Ported from `session-start.sh`, `check-toolu.sh` and `check-deps.sh` ([#269](https://github.com/Falconiere/toolu/issues/269)) |
| `./dispatch` | PreToolUse and PostToolUse dispatchers: `dispatchPreTool` and `dispatchPostTool` walk native built-in modules then `pre-tools.d`/`post-tools.d` with `dispatch.sh` semantics. (#258](https://github.com/Falconiere/toolu/issues/258), [#259](https://github.com/Falconiere/toolu/issues/259), [docs/registry.md](registry.md#pretooluse-dispatch)) |
| `./gates` | Built-in gates as native `ToolModule`s: `gateStatusModule` and `pushWaiverModule` (PostToolUse), plus `qualityCommands` over a parsed command and `toolExitStatus`/`toolInterrupted`/`toolCommand`, which read a payload as the bash modules' jq did ([#259](https://github.com/Falconiere/toolu/issues/259)) |

## Zod boundary rules

| Envelope | Unknown fields | Missing required | Bad version |
|----------|----------------|------------------|-------------|
| Persisted gate/state JSON | reject (`strict`) | `runtime_failure` | fail closed |
| `toolu.config.json` | reject unknown top-level keys | optional keys may default | unsupported major → fail closed |
| Host event envelope | passthrough extras; re-encode known fields only | fail closed for decide fields | N/A |
| Dispatcher output | reject non-object or invalid JSON | `runtime_failure` | N/A |
| Dispatcher stderr | free text | failure reason on nonzero exit | N/A |

Derive types with `z.infer`. No `any`, unchecked casts, or non-null escapes to bypass validation.

## Decision contract

| Outcome | Effect | Native host encoding |
|---------|--------|-------------------------|
| `allow` | proceed | silent success |
| `ask` | host prompt where supported; otherwise class fallback | `permissionDecision: "ask"` where supported (`packages/toolu-core/src/config/gate-mode.ts`) |
| `deny` | hard block | `permissionDecision: "deny"` |
| `advisory` | context only | Host-specific context |
| `post_block` | post-tool feedback | PostToolUse `decision: "block"` (`packages/toolu-core/src/dispatch/dispatch.ts`) |
| `runtime_failure` | fail closed | malformed output or dispatcher error |

**Ask degradation** (`@toolu/core/host` `supportsAsk` / `degradeAsk`): Codex cannot open a native prompt for a toolu gate. Judgement gates use `ask→advise`; security guardrails use `ask→block` (`packages/toolu-core/src/config/gate-mode.ts`, `plugins/toolu/hooks/docs/gates.md`). Cursor enforces `ask` only on `beforeShellExecution`; Hermes shell hooks cannot prompt. Hosts without a plugin prompt use the same class rules, and `encodeDecision` turns any residual `ask` on a pre-action event into a deny.

**Host layer** (`@toolu/core/host`): detection order is `TOOLU_HOST_OVERRIDE`, a stdin `hook_event_name` only one host uses, Cursor's `CURSOR_VERSION` / `CURSOR_PROJECT_DIR`, Codex's `PLUGIN_ROOT`, then Claude. Cursor and Hermes are detected and encoded only; no Cursor manifest or Hermes shim ships yet.

**Precedence:** deny beats ask beats advisory merge; multi-file patches hold ask while walking and emit the first deny immediately (`@toolu/core/dispatch`).

## Event vocabulary

Normalized: `session/{start,resume,clear,unload}`, `prompt`, `pre_compact`/`compaction`, `permission/evaluate`, `tool/{pre,post}`, `shell/pre`, with `sessionId`, `toolCallId`, `cwd`, `projectRoot`, `worktree`, multi-file edit records, cancellation, state scope.

## Policy split (shared with #209)

Classification vocabulary (only these tokens):

- `shell-out`
- `port-native`
- `port-new`
- `no-map`

Implementation choice is orthogonal to host interception. Required unsupported enforcement is a **release blocker**, never `no-map`.

Exhaustive per-source rows: [docs/gate-coverage-matrix.md](gate-coverage-matrix.md) (checked by `bun run tooling/src/gate-coverage-inventory.ts check`).

## Protected-files gate trace

**Fixture:** `fixtures/portable-core/protected-files-pre.json` (Edit targeting `/repo/.env`).

1. Host carries an edit (or Bash write) to a protected path.
2. Normalize via edit records (`packages/toolu-core/src/state/edit-records.ts`).
3. The native protected-files gate (`packages/toolu-core/src/gates/protected-files.ts`, #260; it replaced `protected-files.sh`) + `gateMode(config, "protectedFiles")`, the port of `toolu_gate_mode` in `gate-mode.sh`.
4. Mode `block` → decision `deny`. The host's PreToolUse hook **must** deny **before** the write, so no bytes change. Prompt text alone is not enforcement.
5. Post-tool cannot un-write a completed edit.

## Release blockers (parity)

1. Any required enforcement capability that a supported host cannot provide.
2. Treating prompt context or prompt rewriting as a substitute for a PreToolUse deny.
3. A hook that can crash open: an internal error on an enforcing event must block, never run the tool unguarded.

## Related docs

- Runtime gate modes: [plugins/toolu/hooks/docs/gates.md](../plugins/toolu/hooks/docs/gates.md)
- Config schema: [docs/config.md](config.md)
- Tracking: [#205](https://github.com/Falconiere/toolu/issues/205) / epic [#203](https://github.com/Falconiere/toolu/issues/203)
