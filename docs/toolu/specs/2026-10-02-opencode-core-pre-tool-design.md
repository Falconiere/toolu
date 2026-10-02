# OpenCode core pre-tool enforcement — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Codex   **Topic:** OP-04, issue #338

## Problem

OpenCode's `tool.execute.before` entry currently calls the nine core modules for normalized tools, but does not restrict registry modules to the enabled plugin set and does not invoke the standalone MCP or agent-tier hooks. A stale disabled module can run, and a task model mismatch is ignored. The pinned host runs already-allowed tools unless this hook throws, so these gaps can leave protected operations with side effects.

## Non-Goals

1. Native permission prompts and advisory delivery are #339. Until then, a remaining `ask` from any pre-tool path throws its reason.
2. Post-tool checks and feedback are #340. This issue changes only pre-execution enforcement.
3. Plugin startup completeness, path migration, and catalog installation are later epic work. Use the selection and bootstrap already available.

## Architecture

Retain the #336 `tool.execute.before` hook and #337 payload mapper. Ordinary normalized calls continue through `dispatchPreTool`, which already runs the nine native modules, then `pre-tools.d`, with the established order, per-path patch walk, and deny over ask over advisory behavior. Pass the selected plugin specs from `prepareEnforcement` through the decider to an optional registry allowlist. It applies only when supplied, so Claude and Codex keep their current registry behavior. Registry files from earlier OpenCode sessions may remain on disk but are inert when their spec is not selected.

MCP requests call the standalone `mcpHook`, matching the existing `mcp__` hook's policy and avoiding a second registry walk. Task requests call ordinary dispatch and then `agentTierHook`. Copy the host task's optional `model` and `reasoning_effort` into the core request. Compare the two outcomes with deny/runtime failure before ask before advisory before allow. Execute the task hook at most once for each task call. The core hook's documented fail-open behavior on telemetry/ledger errors remains intact; a real mismatch in block mode throws before the task starts. A real pinned-host probe confirmed that `tool.execute.before` receives a supplied `task.model` even though the standard captured task call omitted it.

The host API remains typed from `@opencode-ai/plugin@1.18.34`. A throw in `tool.execute.before` is the verified pre-side-effect abort. #336's deny-all hook handles setup failures. Do not add another OpenCode gate runner or modify the core gate modules.

## Interfaces / Schema

- `DispatchOptions.selectedRegistrySpecs?: ReadonlySet<string>` and `RunRegistryOptions.selectedSpecs?: ReadonlySet<string>` restrict registry admission before import. With no set, existing behavior is unchanged. An empty set runs no registry modules.
- `PermissionEvaluateHandlerOptions.selectedPluginSpecs?: ReadonlySet<string>` carries the OpenCode selection to `createGateDecider`; the evaluate-shaped compatibility caller may omit it.
- `prepareEnforcement` constructs the set from `selectPluginsWithDependencies(...).plugins[].spec` after successful bootstrap.
- `createGateDecider.decide(request)` routes `tool_name` beginning `mcp__` through `mcpHook`, other requests through `dispatchPreTool`; task decisions also include `agentTierHook` over the same normalized request. All routes return the existing `Decision` union. No new persisted state or public plugin option is added.
- `mapToolCall(task)` preserves optional string `model` and `reasoning_effort` in `tool_input`; supplied non-string values fail closed. Existing required task fields and session/call IDs remain unchanged.

## Failure modes and edge cases

| Input or state | Observable behavior |
| --- | --- |
| Malformed known tool or missing session/call ID | The hook throws `toolu: ...` before any gate or tool runs. |
| Unknown unrelated tool | The adapter skips toolu gates; native host permissions still apply. |
| Dispatcher or standalone hook throws / returns invalid output | Convert to `runtime_failure`; the before hook throws. |
| Gate deny or ask | The before hook throws the gate reason. Ask remains fail-closed until #339. |
| Registry file for an unselected or disabled plugin | Not imported or invoked, even if left by an earlier bootstrap. |
| Multiple registry files for one selected plugin | Preserve the core registry's lexical order, shadowing and per-module run count. |
| Multi-file patch | Preserve core per-path walk; the first deny stops the whole host patch before any file changes. |
| Task with no model | Preserve the core agent-tier rule: missing model inherits the ledger tier, so no synthetic mismatch is raised. |
| Task with model mismatch and agent-tier `block` | The task hook's denial wins over ordinary allow/advisory/ask; no child session starts. |
| Task telemetry or ledger read failure | Preserve the standalone hook's fail-open behavior for this one policy; ordinary core gates still decide. |
| Concurrent or child sessions | Use the host's distinct `sessionID` and `callID` in both paths, with no process-wide per-call mutable state. |
| MCP server configured for the project | The #337 name mapping gives `mcp__<server>__<tool>` to `mcpHook`; a block denies before server invocation. |

## Acceptance criteria

- **AC-1:** Given a selected `ast-grep@toolu` registry module and a matching Grep call, ordinary dispatch uses the same ordered nine native module names as Claude/Codex, then runs selected registry modules once per call; a stale disabled module does not run. A native deny still stops later modules and the whole patch.
- **AC-2:** In an isolated real OpenCode 1.18.34 project with native permissions allowing the calls, protected edit/write/patch, unsafe bash, a commit or push with failing gate/review state, and a blocklisted MCP call fail with toolu reasons and leave their target files or server marker unchanged. An allowed bash call completes.
- **AC-3:** A task with a supplied model that conflicts with a running ledger step and `agentTier: block` fails before the pinned host creates a child session or child side effect. The same task with no model retains the core inherited-tier behavior. The normalized task preserves its session/call IDs, model and effort.
- **AC-4:** A denied core, MCP, or task decision wins over an ask or advisory; malformed input and dispatcher failures refuse execution. An unrelated tool retains native host behavior.
- **AC-5:** `bun run test` and the focused live-host smoke pass without changing Claude/Codex trace results, package surfaces, or the pinned SDK contract.

## Acceptance evidence

| AC | Representative real input and expected observable result | Boundary / failure case | Runnable check |
| --- | --- | --- | --- |
| AC-1 | Compare the decider's ordered `nativeGates` names directly with the canonical `BUILTIN_MODULES` list, exercise a real protected-file deny and the dispatch golden traces, then use the committed `ast-grep` search-nudge bundle and an on-disk fixture registry module that appends one line per invocation. Selected specs write one line per call, unselected specs write none. | A stale disabled registry file remains on disk; an empty selected set imports none; deny short-circuits later modules. | `bun test tools/toolu-opencode/src/adapter/__tests__ packages/toolu-core/src/dispatch/__tests__`; `bun run test:conformance` |
| AC-2 | Scripted loopback provider calls actual host `write`, `edit`, `apply_patch`, `bash`, and configured `probe_touch` MCP tool against an isolated git project; denied tool states show gate reasons, file bytes and MCP marker remain unchanged, allowed marker exists. | Native permission is `allow`; each denial precedes side effects; a combined shell command starts with a marker write before its protected git action. | `bun run smoke:opencode-pretool` |
| AC-3 | A scripted real host task call includes `model: wrong-model`; temp ledger's running step declares another model and mode `block`; hook sees the model, task error names the tier mismatch, child marker/session absent. A direct real-project core-hook test covers absent model. | The host's default task shape omits model, while a supplied model was observed reaching the hook in a real OpenCode run. | `bun run smoke:opencode-pretool`; `bun test tools/toolu-opencode/src/adapter/__tests__` |
| AC-4 | Real temp project, committed modules and malformed host-shaped inputs; test rejects before tool effects and checks deny priority from core plus task policy. | Unknown tool bypasses toolu only; init failure still uses #336 deny-all. | `bun test tools/toolu-opencode/src/adapter/__tests__ tools/toolu-opencode/src/plugin/__tests__` |
| AC-5 | Repository quality and existing-host conformance fixtures; exact pinned host loader and SDK declarations. | Live smoke is opt-in until #362 adds it to CI. | `bun run test`; `bun run check:opencode-host`; `bun run smoke:opencode-entry` |

## Documentation impact

Update `docs/opencode.md` and `docs/portable-core.md` to describe complete pre-tool routing, selected registry modules, and the MCP/task hook behavior under the exact pin. Keep the broader migration guide with #363. `AGENTS.md` already identifies the adapter and core entries; update it only if a key-file description becomes stale.

## Open Questions

None blocking. The host task-model probe resolves the previously uncertain payload field. The #339 ask/advisory policy and #340 post-tool path remain owned by those issues.

## Spec review

- Acceptance evidence: 🔴 blocker (resolved): the first AC-1 check could count registry calls but could not prove the native list or order. AC-1 now requires a direct ordered-name comparison with the canonical list plus existing dispatch behavioral and golden-trace checks.
- Requirement alignment: Jev rated collective coverage 0.70 and scope fit 0.71. Direct inspection found the named real-host checks for side effects, MCP and task denials, plus the full existing-host gate; neither score substitutes for running them.
- **Status:** Approved. Each AC names an observable result, a boundary case and a runnable check; no blocking question remains.
