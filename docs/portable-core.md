# Portable Bun/TS core contracts

**Issue:** [#205](https://github.com/Falconiere/toolu/issues/205) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Status:** `@toolu/core` provides Zod contracts, native gates and dispatchers. Fixture-suite conformance evidence lives in `@toolu/conformance`; see [`conformance-report.md`](conformance-report.md).

The OpenCode target is the documented plugin API. [`opencode-host-contract.md`](opencode-host-contract.md) pins it and records a live probe of every claim ([#335](https://github.com/Falconiere/toolu/issues/335), epic [#334](https://github.com/Falconiere/toolu/issues/334)).

`@toolu/opencode` implements that contract ([#336](https://github.com/Falconiere/toolu/issues/336)). Its root export is a default `PluginModule` whose `server` runs preflight, plugin selection and bootstrap, then returns `tool.execute.before` and `tool.execute.after`. The before hook runs the nine native gates and selected registry modules through `dispatchPreTool`, routes MCP calls through the standalone `mcpHook`, and adds `agentTierHook` to task calls. The after hook appends matching pre-tool advice to a successful tool result. The plugin never throws from init, because the host would then run tools unguarded. Instead, a setup failure returns a hook that refuses every tool call and logs `toolu: not ready: <reason>` to the host log.

## Pins

| Component | Pin | Discovery |
|-----------|-----|-----------|
| OpenCode CLI | `opencode-ai@1.18.34` | npm `opencode-ai`. `bun run probe:opencode-host` installs it into a version-keyed cache; `TOOLU_OPENCODE_HOST_BIN` must report the same version |
| Plugin SDK | `@opencode-ai/plugin@1.18.34` | npm. The host provisions the SDK at its own version; `@toolu/opencode` pins it as a devDependency |
| Bun | `1.4.2` (workspace requires 1.4.x) | `bun --version` |
| Docs baseline | Plugin function returning `Hooks` | https://opencode.ai/docs/plugins/ |
| Contract | Pins, loader behavior, host surface, probes, 16-plugin matrix | [`opencode-host-contract.md`](opencode-host-contract.md), checked by `bun run check:opencode-host` |

Runtime contract: [`runtime.md`](runtime.md). OpenCode requires Bun 1.4.x and git; bash and jq are not prerequisites. Windows is out of scope until probed.

## Package boundaries

| Path | Owns | Must not own |
|------|------|--------------|
| `packages/toolu-core/` | Zod schemas, normalization, decisions, policy and dispatch | Host SDKs (`@opencode-ai/plugin`, `@opencode-ai/sdk`, `@opencode/plugin`), generated skills |
| `tools/toolu-opencode/` | OpenCode `setup`, hooks, generators, host config | Duplicated Zod contracts |
| Conformance CLI (`tools/toolu-conformance/`) | Second real consumer of core exports | Host SDK |

Root: one `bun.lock`, Bun-only scripts/tests, frozen installs. Unified release `vX.Y.Z` across root, packages, and every `plugin.json`. Bun 1.4.x is the documented prerequisite for every host, including Claude Code and Codex (epic [#247](https://github.com/Falconiere/toolu/issues/247) retires the #203 no-Bun constraint); see [`runtime.md`](runtime.md). OpenCode preflight requires git and Bun. Its package contains manifests, settings and committed bundles; selected plugins without a native registration bundle remain NotReady.

TS quality foundation (oxlint/oxfmt, strict `tsc`, structural guardrails, knip, jscpd, Zod-only validators) is adopted in [#213](https://github.com/Falconiere/toolu/issues/213) — see [`docs/conventions-adoption.md`](conventions-adoption.md). Bun workspaces (`packages/toolu-core`, `tools/toolu-opencode`, `tools/toolu-conformance`) and the mandatory CI `typescript` job land in [#208](https://github.com/Falconiere/toolu/issues/208): `bun run test:ts` must pass before runtime implementation merges. Missing Bun/lockfile/tooling fails closed (no successful skip).

### Core export map

| Export | Responsibility |
|--------|----------------|
| `./decision` | Discriminated decision union |
| `./events` | Normalized event schemas + parsers |
| `./policy` | Classification enum + precedence helpers |
| `./config` | `toolu.config.json` Zod (`version: 1`); loader and resolvers ported from the bash config libs: `loadConfig` (jq `*` merge, fail-closed envelope), `enabled`/`model`/`codexModel`/`configString`, `qualityThreshold`, `docsSync*`, `gateMode`/`gateDecision`, `permissionsAutowrite`, and typed `settings/*` loaders ([#253](https://github.com/Falconiere/toolu/issues/253)) |
| `./state` | Persisted state ported from the bash libs, byte-compatible for v1: `recordGateFailure`/`clearGateFile`/`readGateFile` (multi-slot gate file, atomic temp-and-rename writes under `<gate>.lock`, strict `GateFileSchema` with implicit version 1), `sweepState` (same TTL, merge and retention rules), `diffSha`, `telemetryAppend` (closed per-event `TELEMETRY_EXTRAS`, so no free-form payload is ever logged) and `normalizeEditRecords` (Edit, Write, MultiEdit, `apply_patch`) ([#255](https://github.com/Falconiere/toolu/issues/255)) |
| `./registry` | Cross-plugin hook modules as bundled ESM (`<config>/toolu/<dir>.d/<spec>__<name>.js`): `RegistryModule` contract, `runRegistry` (byte-order walk, installed-plugin gating, optional host-selected spec filter, per-module error isolation, stop after deny/block), `registerModules`/`runRegisterHook` (atomic sync, same-prefix prune) and `pruneInactiveModules` (Codex snapshot); parity with `registry.sh`, `dispatch.sh` and the plugins' `register.sh` ([#257](https://github.com/Falconiere/toolu/issues/257), [docs/registry.md](registry.md)) |
| `./ledger` | The delivery-flow plan ledger, the push verdict and push-review waivers. For v1 files they retain parity with the historical Bash implementation. `ledgerMain`/`ledgerRun`/`ledgerStatus`/`ledgerPreflight` (bundled CLI `plugins/toolu/hooks/dist/plan-ledger.js`; each step check runs in its own process group under a native `PLAN_LEDGER_STEP_TIMEOUT`), `parseSteps`/`docField`/`parseAcs`/`checkAcRefs`, `verdictMain`/`verdictReport` (bundled CLI `plugins/toolu/hooks/dist/verdict.js`; quality, plan, review v2, docs), and `pushWaiverPend`/`pushWaiverPromote`/`pushWaiverMatches` ([#256](https://github.com/Falconiere/toolu/issues/256)) |
| `./startup` | What the leaf plugins' SessionStart hooks share: `publishWrapper` (symlink a plugin file at `<config root>/<dir>/<name>`, never over a user's file), `bunOnPath`/`bunAdvisory`, `sessionContext` (bounded to `MAX_CONTEXT_CHARS`) and `renderHookOutput`, and the Codex dependency check `codexMissingPlugins`/`codexDependencyNotice` with host-native install commands. Ported from `session-start.sh`, `check-toolu.sh` and `check-deps.sh` ([#269](https://github.com/Falconiere/toolu/issues/269)) |
| `./dispatch` | PreToolUse and PostToolUse dispatchers: `dispatchPreTool` and `dispatchPostTool` walk native built-in modules then `pre-tools.d`/`post-tools.d` with `dispatch.sh` semantics. The OpenCode package contains only `.js` registry modules ([#258](https://github.com/Falconiere/toolu/issues/258), [#259](https://github.com/Falconiere/toolu/issues/259), [docs/registry.md](registry.md#pretooluse-dispatch)) |
| `./gates` | Built-in gates as native `ToolModule`s: `gateStatusModule` and `pushWaiverModule` (PostToolUse), plus `qualityCommands` over a parsed command and `toolExitStatus`/`toolInterrupted`/`toolCommand`, which read a payload as the bash modules' jq did ([#259](https://github.com/Falconiere/toolu/issues/259)) |

### OpenCode export map ([#211](https://github.com/Falconiere/toolu/issues/211))

| Export | Responsibility |
|--------|----------------|
| `@toolu/opencode/host` | `detectHost`; the global config root (`TOOLU_CONFIG_DIR`, `TOOLU_OPENCODE_HOME`, `$XDG_CONFIG_HOME/opencode`) and per-project data roots (`.opencode/toolu/state/`, or keyed under an override) |
| `@toolu/opencode/inventory` | Installed/enabled/absent/unknown; selection via `.opencode/toolu/plugins.json` + `toolu.config` skills |
| `@toolu/opencode/select` | Enabled set + manifest dependency closure |
| `@toolu/opencode/bootstrap` | `bootstrapRuntime` → Ready \| NotReady with registry/session artifacts |
| `@toolu/opencode/preflight` | git/bun/opencode probe; missing git or Bun fails closed |
| `@toolu/opencode/lifecycle` | Event → supported \| deferred \| unsupported (evaluate wired in #204; other events may stay deferred) |
| `@toolu/opencode` (`./plugin`) | Default `PluginModule` `{ id: "toolu", server }` ([#336](https://github.com/Falconiere/toolu/issues/336)). `server` binds `directory`, `worktree` and `client`, then runs preflight, selection and bootstrap, and returns `tool.execute.before`, `tool.execute.after` and `dispose`. A failed setup returns a deny-all before hook. One instance per directory enforces; a duplicate load (npm spec plus local shim) returns no hooks. `toolu: ready` / `not ready` / `duplicate load skipped` go to the host log |
| `@toolu/opencode/adapter/tool-before` | `mapToolCall` maps `bash`, `read`, `grep`, `glob`, `edit`, `write`, `apply_patch`, `task`, and configured MCP ids onto the core request ([#337](https://github.com/Falconiere/toolu/issues/337)); other tools pass through. `createToolBeforeHandler` throws on deny, runtime failure or a residual ask; allow and advisory return normally, preserving native permission checks. `createDenyAllToolBefore` |
| `@toolu/opencode/adapter/tool-advice` | Per-instance pending advice keyed by session and call ID; `tool.execute.after` appends it only to the matching successful tool result, with bounded storage and expiry |
| `@toolu/opencode/adapter/evaluate` | `createGateDecider`: nine native gates plus selected registry specs through in-process `dispatchPreTool`; MCP through `mcpHook`; task through dispatch plus `agentTierHook`, with deny/runtime failure ahead of ask and advisory. It also exports the retained `permission.evaluate`-shaped `createPermissionEvaluateHandler`, which only the conformance suite and clean-install smoke drive |
| `@toolu/opencode/adapter/permission-map` | Retained `permission.evaluate`-shaped compatibility mapping; never weakens an existing `deny` or `ask` effect |

### OpenCode surface generator ([#206](https://github.com/Falconiere/toolu/issues/206))

| Artifact | Role |
|----------|------|
| `tools/toolu-opencode/scripts/generate-surface.ts` | Deterministic skills/agents/commands mirror under `tools/toolu-opencode/generated/` |
| `docs/portable-frontmatter.md` | Frontmatter preserve/map/reject table |
| `bun run generate:opencode-surface` | Regenerate committed tree |
| `bun run check:opencode-surface` | Fail if sources drift without regen |

Phase 1 enables the `toolu` plugin only (override with `--enabled`); `${CLAUDE_PLUGIN_ROOT}` becomes the owning plugin's `${TOOLU_PLUGIN_ROOT_<PLUGIN>}` in bodies.

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
| `advisory` | context only | Host-specific context; OpenCode appends advice to the matching tool result |
| `post_block` | post-tool feedback | PostToolUse `decision: "block"` (`packages/toolu-core/src/dispatch/dispatch.ts`) |
| `runtime_failure` | fail closed | malformed output or dispatcher error |

**Ask degradation** (`@toolu/core/host` `supportsAsk` / `degradeAsk`): Codex and OpenCode cannot open a native prompt for a toolu gate. Judgement gates use `ask→advise`; security guardrails use `ask→block` (`packages/toolu-core/src/config/gate-mode.ts`, `plugins/toolu/hooks/docs/gates.md`). OpenCode's own `opencode.json` permission asks still prompt through the host; the plugin's declared `permission.ask` hook is never invoked (probe `permission.ask-hook`). Cursor enforces `ask` only on `beforeShellExecution`; Hermes shell hooks cannot prompt. Hosts without a plugin prompt use the same class rules, and `encodeDecision` turns any residual `ask` on a pre-action event into a deny.

**Host layer** (`@toolu/core/host`): detection order is `TOOLU_HOST_OVERRIDE`, the in-process OpenCode flag, a stdin `hook_event_name` only one host uses, Cursor's `CURSOR_VERSION` / `CURSOR_PROJECT_DIR`, Codex's `PLUGIN_ROOT`, then Claude. Cursor and Hermes are detected and encoded only; no Cursor manifest or Hermes shim ships yet.

**Precedence:** deny beats ask beats advisory merge; multi-file patches hold ask while walking and emit the first deny immediately (`@toolu/core/dispatch`).

## Event vocabulary

Normalized: `session/{start,resume,clear,unload}`, `prompt`, `pre_compact`/`compaction`, `permission/evaluate`, `tool/{pre,post}`, `shell/pre`, with `sessionId`, `toolCallId`, `cwd`, `projectRoot`, `worktree`, multi-file edit records, cancellation, state scope.

## OpenCode dispatch contract

Under the documented contract the pre-tool source is `tool.execute.before` (`input.tool`, `input.sessionID`, `input.callID`, `output.args`). A deny is a thrown error, which stops the call before any side effect. The adapter copies `sessionID` and `callID` into the core request and maps `bash`, `read`, `grep`, `glob`, `edit`, `write`, `apply_patch`, `task`, and MCP ids `sanitize(server)_<tool>` (servers from `opencode.json`) onto the core tool names. A `bash` `workdir` becomes that call's `cwd`; otherwise the cwd is the plugin directory. `dispatchPreTool` runs the native core gates followed by `pre-tools.d` modules belonging to the selected OpenCode plugin specs; a stale disabled file is never imported. MCP calls use the standalone `mcpHook` and no ordinary registry walk. Task calls also run `agentTierHook` over their preserved `model` and `reasoning_effort` fields, recording one delegation and blocking a model mismatch in `agentTier: block` mode; an absent model inherits the plan tier. `TOOLU_CONFIG_DIR` points to the OpenCode data root, where selected Bun registration bundles publish registry modules. Deny or runtime failure wins over ask and advisory; residual ask also throws. An allow returns without changing the host's `deny` or `ask` decision. Before-hook advice is held per instance by session and call ID, then appended by `tool.execute.after` to the matching successful result. Missing gated fields, malformed output or failed bootstrap denies. A startup bundle exit 0 does not prove registry readiness; the bootstrap verifies tangible artifacts.

## Policy split (shared with #209)

Classification vocabulary (only these tokens):

- `shell-out`
- `port-native`
- `port-new`
- `no-map`

Implementation choice is orthogonal to host interception. Required unsupported enforcement is a **release blocker**, never `no-map`.

Exhaustive per-source rows: [docs/gate-coverage-matrix.md](gate-coverage-matrix.md) (checked by `bun run tooling/src/gate-coverage-inventory.ts check`).

## Protected-files gate trace

**Fixture:** `tooling/fixtures/portable-core/protected-files-pre.json` (Edit targeting `/repo/.env`).

1. Host carries an edit (or Bash write) to a protected path.
2. Normalize via edit records (`packages/toolu-core/src/state/edit-records.ts`).
3. The native protected-files gate (`packages/toolu-core/src/gates/protected-files.ts`, #260; it replaced `protected-files.sh`) + `gateMode(config, "protectedFiles")`, the port of `toolu_gate_mode` in `gate-mode.sh`.
4. Mode `block` → decision `deny`. The OpenCode adapter **must** throw from `tool.execute.before` **before** the write. A throw blocks `write`, `edit` and multi-file `apply_patch` with no bytes changed (probes `deny.write`, `deny.edit`, `deny.apply-patch`). Prompt text alone is not enforcement.
5. Post-tool cannot un-write a completed edit.

## OpenCode interception (capability table)

Every row is backed by live probes on the pinned host; [`opencode-host-contract.md`](opencode-host-contract.md#probe-results) has the full results.

| Mechanism | Hard-block? | Role for toolu |
|-----------|-------------|----------------|
| `tool.execute.before` (throw) | **Yes** — no side effect, the reason reaches the model | Primary deny path for bash, edit, write, `apply_patch`, grep, MCP (`<server>_<tool>`) and `task`/child-session tools |
| `opencode.json` `permission` rules | **Yes** (the user's rules) | Authoritative user choice. `deny` removes the tool and a plugin cannot override it. `tool.execute.before` runs before the native prompt |
| `permission.ask` | Declared, never invoked | Not usable; gate `ask` degrades by class rules while native `opencode.json` asks remain active |
| `tool.execute.after` | No (post) | Matching pre-tool advice is appended to the successful tool result; post-tool gates belong to OP-06. Not called for a thrown tool error |
| `chat.message`, `experimental.chat.system.transform`, `experimental.session.compacting` | No | Context only (prompt, session, compaction); never enforcement |

### Capability-results

The pinned results, the probe harness and the 16-plugin capability matrix live in [`opencode-host-contract.md`](opencode-host-contract.md). `bun run probe:opencode-host` refreshes the live evidence (`tools/toolu-opencode/contract/probe-results.json`). `bun run check:opencode-host` (part of `bun run test`) keeps that evidence, the matrix, the catalog manifests, the pinned SDK declarations and the contract doc in agreement.

## Release blockers (parity)

1. Any required enforcement capability that the pinned host cannot support. It fails `check:opencode-host` unless it is flagged `releaseBlocker` or has an alternative backed by supported probes.
2. Treating `chat.message`, `experimental.*` context hooks or prompt rewriting as a substitute for a `tool.execute.before` deny.
3. A plugin init that can throw. The loader is fail-open: it logs `failed to load plugin` and runs tools unguarded (probe `load.init-throw`).
4. Advertising OpenCode enforcement when bootstrap or the pin check fails.

## Related docs

- Runtime gate modes: [plugins/toolu/hooks/docs/gates.md](../plugins/toolu/hooks/docs/gates.md)
- Config schema: [docs/config.md](config.md)
- OpenCode host contract: [opencode-host-contract.md](opencode-host-contract.md) ([#335](https://github.com/Falconiere/toolu/issues/335))
- Tracking: [#205](https://github.com/Falconiere/toolu/issues/205) / epic [#203](https://github.com/Falconiere/toolu/issues/203)
