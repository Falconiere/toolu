# Portable Bun/TS core contracts

**Issue:** [#205](https://github.com/Falconiere/toolu/issues/205) (epic [#203](https://github.com/Falconiere/toolu/issues/203))\
**Status:** `@toolu/core` provides Zod contracts, native gates and dispatchers. Fixture-suite conformance evidence lives in `@toolu/conformance`; see [`conformance-report.md`](conformance-report.md).

The OpenCode target is the documented plugin API. [`opencode-host-contract.md`](opencode-host-contract.md) pins it and records a live probe of every claim ([#335](https://github.com/Falconiere/toolu/issues/335), epic [#334](https://github.com/Falconiere/toolu/issues/334)).

The shipped `@toolu/opencode` predates that contract. It still calls `dispatchPreTool` from the superseded `Plugin.define` / `permission.hook("evaluate")` entry of `@opencode/plugin@2.0.12` ([#276](https://github.com/Falconiere/toolu/issues/276)). OP-02 ([#336](https://github.com/Falconiere/toolu/issues/336)) replaces that entry.

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
| `./registry` | Cross-plugin hook modules as bundled ESM (`<config>/toolu/<dir>.d/<spec>__<name>.js`): `RegistryModule` contract, `runRegistry` (byte-order walk, installed-plugin gating, per-module error isolation, stop after deny/block), `registerModules`/`runRegisterHook` (atomic sync, same-prefix prune) and `pruneInactiveModules` (Codex snapshot); parity with `registry.sh`, `dispatch.sh` and the plugins' `register.sh` ([#257](https://github.com/Falconiere/toolu/issues/257), [docs/registry.md](registry.md)) |
| `./ledger` | The delivery-flow plan ledger, the push verdict and push-review waivers. For v1 files they retain parity with the historical Bash implementation. `ledgerMain`/`ledgerRun`/`ledgerStatus`/`ledgerPreflight` (bundled CLI `plugins/toolu/hooks/dist/plan-ledger.js`; each step check runs in its own process group under a native `PLAN_LEDGER_STEP_TIMEOUT`), `parseSteps`/`docField`/`parseAcs`/`checkAcRefs`, `verdictMain`/`verdictReport` (bundled CLI `plugins/toolu/hooks/dist/verdict.js`; quality, plan, review v2, docs), and `pushWaiverPend`/`pushWaiverPromote`/`pushWaiverMatches` ([#256](https://github.com/Falconiere/toolu/issues/256)) |
| `./startup` | What the leaf plugins' SessionStart hooks share: `publishWrapper` (symlink a plugin file at `<config root>/<dir>/<name>`, never over a user's file), `bunOnPath`/`bunAdvisory`, `sessionContext` (bounded to `MAX_CONTEXT_CHARS`) and `renderHookOutput`, and the Codex dependency check `codexMissingPlugins`/`codexDependencyNotice` with host-native install commands. Ported from `session-start.sh`, `check-toolu.sh` and `check-deps.sh` ([#269](https://github.com/Falconiere/toolu/issues/269)) |
| `./dispatch` | PreToolUse and PostToolUse dispatchers: `dispatchPreTool` and `dispatchPostTool` walk native built-in modules then `pre-tools.d`/`post-tools.d` with `dispatch.sh` semantics. The OpenCode package contains only `.js` registry modules ([#258](https://github.com/Falconiere/toolu/issues/258), [#259](https://github.com/Falconiere/toolu/issues/259), [docs/registry.md](registry.md#pretooluse-dispatch)) |
| `./gates` | Built-in gates as native `ToolModule`s: `gateStatusModule` and `pushWaiverModule` (PostToolUse), plus `qualityCommands` over a parsed command and `toolExitStatus`/`toolInterrupted`/`toolCommand`, which read a payload as the bash modules' jq did ([#259](https://github.com/Falconiere/toolu/issues/259)) |

### OpenCode export map ([#211](https://github.com/Falconiere/toolu/issues/211))

| Export | Responsibility |
|--------|----------------|
| `@toolu/opencode/host` | `detectHost`, OpenCode data/config roots (`TOOLU_OPENCODE_HOME`, `TOOLU_CONFIG_DIR`, `.opencode/`) |
| `@toolu/opencode/inventory` | Installed/enabled/absent/unknown; selection via `.opencode/toolu/plugins.json` + `toolu.config` skills |
| `@toolu/opencode/select` | Enabled set + manifest dependency closure |
| `@toolu/opencode/bootstrap` | `bootstrapRuntime` → Ready \| NotReady with registry/session artifacts |
| `@toolu/opencode/preflight` | git/bun/opencode probe; missing git or Bun fails closed |
| `@toolu/opencode/lifecycle` | Event → supported \| deferred \| unsupported (evaluate wired in #204; other events may stay deferred) |
| `@toolu/opencode/plugin` | Current default entry (superseded `Plugin.define`): preflight, bootstrap, `permission.hook("evaluate")`. OP-02 ([#336](https://github.com/Falconiere/toolu/issues/336)) replaces it with the documented plugin function |
| `@toolu/opencode/adapter/permission-map` | OpenCode permission event → PreToolUse payload; decision → effect |
| `@toolu/opencode/adapter/evaluate` | `createPermissionEvaluateHandler` (in-process `dispatchPreTool`) |

### OpenCode surface generator ([#206](https://github.com/Falconiere/toolu/issues/206))

| Artifact | Role |
|----------|------|
| `tools/toolu-opencode/scripts/generate-surface.ts` | Deterministic skills/agents/commands mirror under `tools/toolu-opencode/generated/` |
| `docs/portable-frontmatter.md` | Frontmatter preserve/map/reject table |
| `bun run generate:opencode-surface` | Regenerate committed tree |
| `bun run check:opencode-surface` | Fail if sources drift without regen |

Phase 1 enables the `toolu` plugin only (override with `--enabled`); `${CLAUDE_PLUGIN_ROOT}` becomes `${TOOLU_PLUGIN_ROOT}` in bodies.

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
| `ask` | host prompt | `permissionDecision: "ask"` (`packages/toolu-core/src/config/gate-mode.ts`) |
| `deny` | hard block | `permissionDecision: "deny"` |
| `advisory` | context only | `additionalContext` / `systemMessage` |
| `post_block` | post-tool feedback | PostToolUse `decision: "block"` (`packages/toolu-core/src/dispatch/dispatch.ts`) |
| `runtime_failure` | fail closed | malformed output or dispatcher error |

**Ask degradation** (`@toolu/core/host` `supportsAsk` / `degradeAsk`): Codex cannot prompt — judgement gates `ask→advise`, security guardrails `ask→block` (`packages/toolu-core/src/config/gate-mode.ts`, `plugins/toolu/hooks/docs/gates.md`). On the documented OpenCode contract a plugin cannot open a native prompt either: the declared `permission.ask` hook is never invoked (probe `permission.ask-hook`). OP-05 ([#339](https://github.com/Falconiere/toolu/issues/339)) moves OpenCode onto the same class rules; until then `supportsAsk` reflects the superseded V2 adapter; Cursor enforces `ask` only on `beforeShellExecution`; Hermes shell hooks cannot prompt. Hosts without prompt use the same class rules, and `encodeDecision` turns any `ask` that still reaches them on a pre-action event into a deny.

**Host layer** (`@toolu/core/host`): detection order is `TOOLU_HOST_OVERRIDE`, the in-process OpenCode flag, a stdin `hook_event_name` only one host uses, Cursor's `CURSOR_VERSION` / `CURSOR_PROJECT_DIR`, Codex's `PLUGIN_ROOT`, then Claude. Cursor and Hermes are detected and encoded only; no Cursor manifest or Hermes shim ships yet.

**Precedence:** deny beats ask beats advisory merge; multi-file patches hold ask while walking and emit the first deny immediately (`@toolu/core/dispatch`).

## Event vocabulary

Normalized: `session/{start,resume,clear,unload}`, `prompt`, `pre_compact`/`compaction`, `permission/evaluate`, `tool/{pre,post}`, `shell/pre`, with `sessionId`, `toolCallId`, `cwd`, `projectRoot`, `worktree`, multi-file edit records, cancellation, state scope.

## OpenCode dispatch contract

Under the documented contract the pre-tool source is `tool.execute.before` (`input.tool`, `output.args`). A deny is a thrown error, which stops the call before any side effect; OP-03 and OP-04 ([#337](https://github.com/Falconiere/toolu/issues/337), [#338](https://github.com/Falconiere/toolu/issues/338)) own that mapping. The current V2 adapter maps `sessionID`, tool-call metadata, action and resources to a Claude-shaped PreToolUse payload, then calls `dispatchPreTool` with the native core gates. `TOOLU_CONFIG_DIR` points to the OpenCode data root, where selected Bun registration bundles publish registry modules. The dispatcher result maps deny, ask and advisory to OpenCode effects. Missing gated fields, malformed output or failed bootstrap denies. A startup bundle exit 0 does not prove registry readiness; the bootstrap verifies tangible artifacts.

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
| `permission.ask` | Declared, never invoked | Not usable; gate `ask` degrades by class rules (OP-05) |
| `tool.execute.after` | No (post) | Feedback appended to the tool output; not called for a thrown tool error |
| `chat.message`, `experimental.chat.system.transform`, `experimental.session.compacting` | No | Context only (prompt, session, compaction); never enforcement |

### Capability-results

The pinned results, the probe harness and the 16-plugin capability matrix live in [`opencode-host-contract.md`](opencode-host-contract.md). `bun run probe:opencode-host` refreshes the live evidence (`tools/toolu-opencode/contract/probe-results.json`). `bun run check:opencode-host` (part of `bun run test`) keeps that evidence, the matrix, the catalog manifests, the pinned SDK declarations and the contract doc in agreement.

## Release blockers (parity)

1. Any required enforcement capability that the pinned host cannot support. It fails `check:opencode-host` unless it is flagged `releaseBlocker` or has an alternative backed by supported probes.
2. Treating `chat.message`, `experimental.*` context hooks or prompt rewriting as a substitute for a `tool.execute.before` deny.
3. A plugin init that can throw. The loader is fail-open: it logs `failed to load plugin` and runs tools unguarded (probe `load.init-throw`).
4. Advertising OpenCode enforcement when bootstrap or the pin check fails.

## Related docs

- Runtime gate modes: [plugins/toolu/hooks/docs/gates.md](../../toolu/hooks/docs/gates.md)
- Config schema: [docs/config.md](config.md)
- OpenCode host contract: [opencode-host-contract.md](opencode-host-contract.md) ([#335](https://github.com/Falconiere/toolu/issues/335))
- Tracking: [#205](https://github.com/Falconiere/toolu/issues/205) / epic [#203](https://github.com/Falconiere/toolu/issues/203)
