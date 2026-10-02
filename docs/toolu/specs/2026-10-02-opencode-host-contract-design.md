# OpenCode documented host contract (OP-01) — Design

**Date:** 2026-10-02   **Status:** Draft   **Author:** Claude (epic worker, #335)   **Topic:** Pin the docs-matching OpenCode host and SDK, probe it for real, and check a 16-plugin capability matrix against that evidence.

## Problem

The adapter and its contract docs target `@opencode/plugin@2.0.12`. That V2 API uses `Plugin.define` and `permission.hook("evaluate")`. The requested public API at <https://opencode.ai/docs/plugins/> is a plugin function that returns `Hooks` (`@opencode-ai/plugin`). `tooling/src/opencode-capability-probe.ts` writes a hardcoded V2 matrix after checking only `opencode --version`, so no committed evidence shows what the documented host actually does. Nine dependent work packages (OP-02 … OP-29) need a verified contract before they implement anything. Without it they would repeat the old mistake of building against assumed event shapes.

## Non-Goals

1. No adapter change. `tools/toolu-opencode/src/**` keeps its V2 entry until OP-02 (#336). Bootstrap, normalization, gates, post-tool, context delivery, surfaces and the CLI belong to their own work packages.
2. No mandatory live OpenCode run in `bun run test` or CI. OP-28 (#362) owns that. This issue ships the live harness and checks its committed evidence hermetically.
3. No rewrite of install or migration docs (OP-29, #363). `docs/opencode.md`, `docs/conformance-report.md` and `tools/toolu-opencode/README.md` only gain a one-line pointer saying the documented contract lives in `docs/opencode-host-contract.md`.
4. `tooling/src/clean-install-smoke.ts`, which smoke-tests the current V2 adapter and is not part of `bun run test`, stays unchanged until OP-27/OP-28.
5. TUI rendering, Windows, and npm-registry plugin specs (`"plugin": ["pkg@ver"]`) are not probed. They are recorded as unverified, with owners.

## Architecture

**Pin.** The pin is `opencode-ai@1.18.34` (CLI, installed from npm with `bun add --exact`, platform package `opencode-<os>-<arch>`) plus `@opencode-ai/plugin@1.18.34`. These were the latest releases on 2026-10-02, and they are the line documented at `/docs/plugins/`. The host itself writes `@opencode-ai/plugin: <host version>` into each config dir's `package.json`, so the CLI and SDK versions are the same number. Jev preferred this pin over the locally installed `opencode v2.0.21` or keeping `v2.0.12` (1.00). Experimental hooks follow an exact-pin strategy: the adapter may use an `experimental.*` hook only for context delivery, never for enforcement. Every pin bump reruns the live probes and must reproduce the committed verdicts before the pin can change.

**Probe driver.** The live probes run the real pinned binary through `opencode run --format json` (stdin closed) and `opencode serve` (for compaction via `POST /session/:id/summarize`). Each probe gets an isolated `HOME`/`XDG_*` profile and a fresh git project. A loopback scripted OpenAI-compatible provider (`Bun.serve`, SSE `chat.completion.chunk`) returns scripted tool calls; only the model output is scripted. Plugin loading, permissions, tool execution, MCP and child sessions are the host's own code. Jev chose this driver over a credentialed LLM or direct adapter calls (0.98). Side effects are checked on disk, and model-visible results are checked in the requests the provider records.

**Placement.** Jev chose this split (1.00):

- `tools/toolu-opencode/contract/` holds the contract data and the typed probe plugins, which import `@opencode-ai/plugin` types. They sit in the adapter workspace so host SDK types stay in the adapter, but outside the npm `files` list. `@opencode-ai/plugin@1.18.34` becomes an exact `devDependency` of `@toolu/opencode`, and root `tsconfig.json` includes `tools/toolu-opencode/contract/**/*.ts`.
- `tooling/src/opencode-host/` holds the harness modules, which do not import the SDK.
- `tooling/src/opencode-host-probe.ts` is the live CLI (`bun run probe:opencode-host`).
- `tooling/src/opencode-host-contract.ts` is the hermetic checker (`bun run check:opencode-host`). It joins `test:portable-core` and so runs in `bun run test`.

It replaces `opencode-capability-probe.ts`, its test, and `tooling/fixtures/portable-core/capability-probe-results.json`.

**Doc.** A new `docs/opencode-host-contract.md` records the pin, install source, platform, the host surface (hook callbacks vs event notifications vs config surfaces), loader behavior, and the experimental-hook strategy. It has three generated blocks — probe results, the 16-plugin matrix, and release blockers/limitations — that the checker keeps in sync. `docs/portable-core.md` replaces its V2 pins, interception table, capability-results block and release-blocker list with the documented contract and a link. Lint `no-restricted-imports` in core, tooling and conformance adds `@opencode-ai/plugin` and `@opencode-ai/sdk`.

**Reuse.** Tests use `@toolu/conformance/harness/{spawn,sandbox}` for real subprocesses and temp trees. Docs-block handling follows the existing marker pattern in `check-portable-core-doc.ts` and `opencode-capability-probe.ts`.

## Interfaces / Schema

`tools/toolu-opencode/contract/pin.json` (Zod, strict):

```json
{ "version": 1,
  "cli": { "package": "opencode-ai", "version": "1.18.34" },
  "sdk": { "package": "@opencode-ai/plugin", "version": "1.18.34" },
  "docs": "https://opencode.ai/docs/plugins/" }
```

`tools/toolu-opencode/contract/probe-results.json` (Zod, strict; only the live CLI writes it):

```ts
type ProbeResults = {
  version: 1; recordedAt: string /* YYYY-MM-DD */;
  host: { cli: string; cliVersion: string; sdk: string; provisionedSdkVersion: string;
          platform: string /* e.g. linux-x64 */; bun: string; installSource: string };
  probes: Array<{ id: ProbeId; axis: Axis; kind: "hook" | "event" | "loader" | "config" | "surface";
                  mechanism: string; claim: string; verdict: "supported" | "unsupported";
                  observed: Record<string, boolean | string | number> }>;
};
type Axis = "load" | "tools" | "permission" | "startup" | "prompt" | "compaction" | "postTool"
          | "mcp" | "task" | "ui" | "surfaces" | "env";
```

`ProbeId` is the closed list exported by `tooling/src/opencode-host/scenarios.ts`: `load.local-file`, `load.config-file`, `load.module-default`, `load.init-throw`, `load.helper-export`, `deny.bash`, `deny.write`, `deny.apply-patch`, `deny.mcp`, `deny.task-child`, `pre.advisory`, `permission.ask-hook`, `permission.config-deny`, `permission.order`, `post.feedback`, `post.bash-exit`, `post.tool-error`, `context.system`, `context.prompt`, `context.compaction`, `env.shell`, `command.hook`, `surface.files`, `surface.names`, `surface.config-hook`, `ui.toast`, `events.bus`. `observed` holds only deterministic values: booleans, tool names, exit codes, counts. It never holds session ids, paths or timestamps.

`tools/toolu-opencode/contract/capability-matrix.json` (Zod, strict):

```ts
type Matrix = { version: 1; plugins: Record<PluginName, {
  owner: WorkPackage[];                 // epic ownership, e.g. ["OP-16"]
  axes: Record<"tools"|"permission"|"startup"|"prompt"|"compaction"|"postTool"|"mcp"|"task"|"ui", Cell>;
  surfaces: { skills: number; commands: number; agents: number; owner: WorkPackage[] };
  notes?: Array<{ need: string; owner: WorkPackage }>;
}> };
type Cell = { use: "none" } | {
  use: string; required: boolean; mechanism: string; kind: "hook"|"event"|"config"|"tool"|"tui";
  status: "supported" | "partial" | "unsupported"; evidence: ProbeId[]; owner: WorkPackage[];
  alternative?: string; releaseBlocker?: boolean };
type WorkPackage = "OP-02" | … | "OP-29";   // OP-n ↔ issue #(334+n)
```

`bun run check:opencode-host` (`opencode-host-contract.ts check`) exits 0 and prints `opencode-host-contract: ok` only when all of the following hold:

1. `pin.json` parses.
2. The adapter's `devDependencies["@opencode-ai/plugin"]` equals `pin.sdk.version`.
3. `probe-results.json` parses. Its `host.cliVersion` and `host.provisionedSdkVersion` equal the pins, and it contains each `ProbeId` exactly once.
4. The matrix keys equal the `plugins/*/` directories that contain `.claude-plugin/plugin.json`.
5. Manifest-derived uses are non-`none`:
   - `SessionStart` → startup, `UserPromptSubmit` → prompt, `PreCompact` → compaction.
   - A `PreToolUse` matcher naming `Bash|Shell|Edit|Write|MultiEdit|apply_patch|Grep`, or a `tool/pre` registry module → tools.
   - `mcp__` → mcp. `Agent|Task|spawn_agent` → task.
   - `PostToolUse`, or a `tool/post` registry module → postTool.
6. Surface counts equal the on-disk counts: `skills/*/SKILL.md`, `commands/*.md`, `agents/*.md`.
7. Each non-`none` cell names at least one existing evidence id, and its status agrees with the verdicts:
   - `supported` ⇒ every evidence verdict is supported.
   - `unsupported` ⇒ every evidence verdict is unsupported.
   - `partial` ⇒ both verdicts occur.
8. A required cell that is not `supported` has a non-empty `alternative`, or has `releaseBlocker: true`.
9. The three doc blocks equal their rendering.

`check --write-doc` rewrites the doc blocks. Path overrides for tests: `TOOLU_OPENCODE_CONTRACT_DIR`, `TOOLU_OPENCODE_CONTRACT_DOC`, `TOOLU_PLUGINS_DIR`, `TOOLU_OPENCODE_ADAPTER_PKG`.

`bun run probe:opencode-host [--write]` resolves the binary in this order:

1. `TOOLU_OPENCODE_HOST_BIN`, whose `--version` must equal the pin.
2. Otherwise it installs `opencode-ai@<pin>` into `${TOOLU_OPENCODE_HOST_CACHE:-$XDG_CACHE_HOME/toolu/opencode-host}/<pin>`.

It runs every scenario and builds `ProbeResults`. Without `--write` it compares each probe's `verdict` and `observed` with the committed file and exits 1 on drift. With `--write` it rewrites the file. Either way it prints one line per probe: `<id> <verdict>`.

Scripted provider (`tooling/src/opencode-host/provider.ts`): `startScriptedProvider(scripts: Record<string, ScriptStep[]>) → { url, requests(): RecordedRequest[], stop() }`. A request without `tools` gets the text `Probe title`. Otherwise the provider reads the `PROBE:<scenario>` token from the first user message and returns step N, where N is the number of tool messages after the last user message. When the script runs out it returns the text `PROBE-DONE`.

MCP fixture (`tooling/src/opencode-host/mcp-server.ts`): a stdio JSON-RPC server with one tool, `touch {name}`. Each call appends to `$MCP_MARKER`.

## Failure modes and edge cases

- **Pinned CLI cannot be installed** (no network or registry failure) or `TOOLU_OPENCODE_HOST_BIN` reports another version: the probe CLI exits 1 with `opencode-host-probe: <reason>`, writes nothing, and never reports success.
- **A scenario hangs:** each host run has a hard timeout of 120 s, and stdin is always `/dev/null`. On timeout the run is killed, the provider and serve processes are stopped, and the probe CLI exits 1 naming the probe. Every scenario uses a `finally` to stop the provider and server.
- **Verdict drift** on a re-run, for example a host change: exit 1, listing each probe id with its old and new verdict. The committed evidence is not changed without `--write`.
- **Checker inputs:**
  - Missing or invalid JSON, or unknown keys, fail with `opencode-host-contract: <file>: <zod issue>`.
  - A new `plugins/<x>/` without a matrix row fails `missing matrix row: <x>`. A stale row fails `unknown plugin: <x>`.
  - An evidence id absent from the results fails `unknown evidence <id> in <plugin>.<axis>`.
  - A status contradicting the verdicts fails `status <s> contradicts evidence for <plugin>.<axis>`.
  - A required non-supported cell with no alternative or blocker fails `required <plugin>.<axis> is <s> without alternative or releaseBlocker`.
  - Doc drift fails `doc block <name> is stale; run check --write-doc`.
  - A pin mismatch fails `pin mismatch: <what>`.
- **User profile safety:** probes never read or write the real `~/.config/opencode` or `~/.cache/opencode`. Every host process gets temp `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_CACHE_HOME` and `XDG_STATE_HOME`. The real `opencode` on `PATH` is ignored.
- **Concurrency:** each scenario has its own sandbox and provider port. Scenarios run sequentially to keep the host's first-run config-dir install deterministic.

## Acceptance criteria

- **AC-1:** Given the pinned `opencode-ai@1.18.34`, a typed plugin (`Plugin` from `@opencode-ai/plugin`, typechecked by `bun run typecheck`) is loaded from `.opencode/plugins/` and, separately, from `opencode.json` `plugin: [["file://…", options]]`. The live probe records `load.local-file` and `load.config-file` as `supported`, with options delivered only on the config path.
- **AC-2:** Real host runs record how the pinned host handles enforcement and context:
  - Deny before side effects: a `tool.execute.before` throw leaves no file for bash, write, multi-file `apply_patch`, MCP and task-child calls, and its error text reaches the model.
  - Permission composition: `permission.ask` is not invoked, config `deny` cannot be overridden, and `before` precedes `permission.asked`.
  - Context delivery: system, prompt and compaction context reach the provider request.
  - Tool failure: `after` runs for bash exit 3, not for a thrown tool error.

  Each item has a committed verdict.
- **AC-3:** The pinned host's loader constraints are recorded as `unsupported` claims with owner OP-02. On a plugin init throw it fails open: the tool runs unguarded. It invokes every exported function as a plugin, and that breaks prompts.
- **AC-4:** `bun run check:opencode-host` passes on the repository. It fails with the named messages when the matrix misses a `plugins/*` directory, cites an unknown probe id, states a status the evidence contradicts, leaves a required non-supported cell without an alternative or blocker, or drifts from the doc, or when the pin and adapter devDependency disagree.
- **AC-5:** All 16 plugins have a matrix row with an epic owner. Every manifest-derived axis is filled, and surface counts match disk. Every required cell has an owner, evidence and a status. The doc renders the matrix and a release-blocker/limitation list in which each entry names an owner issue.
- **AC-6:** The chosen contract keeps no V2-only assumption. `docs/portable-core.md` cites `opencode-ai@1.18.34` and `@opencode-ai/plugin@1.18.34`, and the V2 pin, `Plugin.define` and `permission.hook("evaluate")` appear there only as the superseded current-adapter state. The hardcoded V2 capability probe and its fixture are gone. The doc lists each `experimental.*` hook the matrix relies on under the exact-pin strategy.
- **AC-7:** The scripted provider and MCP fixture behave as specified over real loopback HTTP and real stdio subprocesses.

## Acceptance evidence

| AC | Real input | Expected observable result | Boundary / failure | Runnable check |
|---|---|---|---|---|
| AC-1 | Pinned host from npm; `contract/plugins/probe.ts`; isolated profile | `load.local-file` and `load.config-file` supported; `observed.optionsDelivered` is false locally and true via config | Version mismatch through `TOOLU_OPENCODE_HOST_BIN` → exit 1 | `bun run probe:opencode-host` (live) and `bun run typecheck` |
| AC-2 | Same host; scripted bash/write/apply_patch/MCP/task scenarios; config `bash: ask` / `deny` | No marker files; error text in the recorded provider request; verdicts as stated | Child session; multi-file patch; `--auto` approval | `bun run probe:opencode-host` (live, compares with the committed results) |
| AC-3 | `contract/plugins/init-throw.ts`, `contract/plugins/helper-export.ts` | Marker present after init throw; prompt error with the helper export; both `unsupported` | Throwing plugin plus a healthy plugin: the healthy one still enforces | `bun run probe:opencode-host` |
| AC-4 | Committed contract files; mutated copies in a sandbox | Exit 0 on the repo; exit 1 with each named message on each mutation | Each listed checker failure | `bun test tooling/src/__tests__/opencode-host-contract.test.ts` |
| AC-5 | Real `plugins/*` manifests and committed matrix | Check passes; doc matrix lists 16 rows | Removing a row, or adding a `plugins/x` with a manifest → named failure | `bun run check:opencode-host` and the test above |
| AC-6 | `docs/portable-core.md`, repo tree | Doc checker passes with the new citations; deleted files absent | Restoring the V2 pin row as the contract → doc check failure | `bun run test:portable-core` |
| AC-7 | `Bun.serve` provider; `bun mcp-server.ts` subprocess | SSE tool call for a scripted step, `Probe title` without tools, `PROBE-DONE` when exhausted; MCP initialize/list/call appends the marker | Unknown scenario → `PROBE-DONE`; malformed JSON line → error response, no crash | `bun test tooling/src/opencode-host/__tests__/` |

## Documentation impact

- New `docs/opencode-host-contract.md`.
- `docs/portable-core.md`: the Pins, OpenCode interception, Capability-results and Release blockers sections change.
- `docs/opencode.md`, `docs/conformance-report.md` and `tools/toolu-opencode/README.md`: each gains a one-line pointer saying the current adapter targets V2 until #336 and the documented contract lives in the new doc.
- `AGENTS.md` key files table: add the contract checker and probe CLIs.
- `docs/testing.md`: one line on the live probe being opt-in until OP-28.
- No skill or command changes.

## Open Questions

1. Should OP-28 run the live probe on macOS too? Owner: OP-28. Non-blocking; this issue records linux-x64 evidence and states its platform.
2. Is `permission.ask` wired only in the TUI/server path of a later host? Owner: OP-05. Non-blocking: the contract records it as not invoked on the pin and requires the ask degradation alternative.
