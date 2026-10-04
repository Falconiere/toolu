# OpenCode statusline native diagnostics — Design

**Date:** 2026-10-04 **Status:** Approved **Author:** Claude Code **Topic:** OP-25 (#359): port statusline to supported OpenCode diagnostics

## Problem

On OpenCode, the generated `statusline-status` skill reports the repository, gate and Jev fields, but nothing about toolu itself. It cannot tell which plugins were selected, whether their startup ran, or why toolu is not ready. That state exists only in the adapter's plugin process. The capability matrix also claims `statusline.ui` support through `client.tui.showToast`, but nothing implements it. The persistent statusline itself is a Claude Code `settings.json` feature with no OpenCode equivalent. The generated catalog still lists the Claude-only setup command as pending OP-25, not as a settled host-specific limitation.

## Non-Goals

1. Ship an OpenCode TUI slot plugin (`@opencode-ai/plugin/tui`), a toast, or any other persistent status UI. The pinned headless harness cannot verify TUI rendering.
2. Ship an OpenCode version of `/statusline:setup`, or write any `statusLine` key or other Claude Code setting into OpenCode configuration.
3. Change the Claude Code renderer, the Codex `$statusline:status` output, or the statusline SessionStart hook.
4. Make the status record an input to enforcement. Readiness and gates are still decided in process. The record is diagnostic only.
5. Detect a stale record by process liveness. The report shows the record's write time and project.

## Architecture

1. **Status record (adapter).** After `createTooluHooks` settles enforcement, a new `tools/toolu-opencode/src/plugin/status-record.ts` writes `<data root>/toolu/opencode-status.json` for both the ready and not-ready verdicts. The duplicate-load branch skips the write because its instance enforces nothing. The data root comes from `opencodeDataRoot({ projectRoot, env })`, the same root bootstrap, the ledger and `shell.env`'s `TOOLU_CONFIG_DIR` use. The write is atomic: a unique temp file is renamed into place, as in `writeLedger`. A failed write produces one `client.app.log` error entry and never changes the verdict.
2. **Structured log (adapter).** `HostBinding.log` gains an optional `extra` argument that is passed through as `body.extra`. Every startup that settles enforcement sends one `info` entry (`error` when not ready) with message `toolu: status`. Its `extra` holds flat fields: `status`, `plugins` (comma-joined names), `selection`, `artifacts`, `record` (the file path), and `reason` when not ready. Existing plain log lines are unchanged.
3. **Shared schema (core).** `@toolu/core/startup` gains `opencode-status.ts`, which exports the record type and zod schema, `OPENCODE_STATUS_FILE`, `opencodeStatusPath(configRoot)` and `readOpencodeStatus(path)`. The adapter writes through the schema. The statusline bundle reads through the same module, so the plugin stays self-contained and depends only on `@toolu/core`.
4. **Status report (statusline).** For host `opencode` only, `collectStatus` reads the record from `opencodeStatusPath(configRoot({ env, host }))`. In OpenCode's bash, `configRoot` is `TOOLU_CONFIG_DIR`, which is the data root. `reportText` then prints toolu lines after `Host:`. Claude and Codex never read the file, so their output stays byte-identical.
5. **Generated surface.** The `statusline-status` rewrite runs `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"`. Both values are set by `shell.env` (#343), so the command works off `PATH` from any cwd and ignores the project `.env`. The description names OpenCode and toolu readiness. The setup command stays excluded, and the reason changes from "pending OP-25" to the final host-specific limitation.
6. **Capability matrix.** `statusline.ui` becomes `{ "use": "none" }`. The statusline note records that the persistent statusline is Claude Code-specific, that the pinned SDK's TUI slots cannot be verified headless, and that the status skill plus `client.app.log` diagnostics are the alternative. `tools.use` drops the setup command. The contract doc's generated blocks are regenerated.

Decisive trade-off: a persisted record against env-only selection. Jev chose the record (choice `snapshot`, 0.91), because bash otherwise cannot see startup entries, artifacts or notes. For the UI it chose `none-plus-note` (0.96) over a toast or a new negative probe.

## Interfaces / Schema

```ts
// packages/toolu-core/src/startup/opencode-status.ts
export const OPENCODE_STATUS_FILE = "opencode-status.json";
export function opencodeStatusPath(configRoot: string): string; // join(configRoot, "toolu", OPENCODE_STATUS_FILE)
export type OpencodeStatusRecord = {
  version: 1;
  written: string;            // ISO-8601
  project: string;            // the instance's project root
  status: "ready" | "not-ready";
  reason?: string;            // not-ready only, ≤ 4 000 chars
  selection?: "project" | "global" | "default"; // ready only
  plugins: { name: string; entries: string[]; artifacts: number }[]; // ready: startup order; not-ready: []
  notes: string[];            // ≤ 20 startup diagnostics, each ≤ 500 chars
};
export type OpencodeStatusRead =
  | { ok: true; record: OpencodeStatusRecord }
  | { ok: false; reason: "missing" | "invalid" };
export function readOpencodeStatus(path: string): OpencodeStatusRead;

// tools/toolu-opencode/src/plugin/status-record.ts
export function statusRecord(enforcement: Enforcement, project: string, now: Date): OpencodeStatusRecord;
export function writeStatusRecord(dataRoot: string, record: OpencodeStatusRecord): string | undefined; // failure reason

// tools/toolu-opencode/src/plugin/context.ts
log: (level: LogLevel, message: string, extra?: Record<string, string | number>) => Promise<void>;
```

`ProjectStatus` gains `toolu: { status: "" | "ready" | "not-ready" | "missing" | "invalid"; path: string; record?: OpencodeStatusRecord }`, which is `""` for Claude and Codex. On OpenCode, the report lines after `Host: OpenCode` are:

```
toolu: ready — 2 plugins (project selection), 3 startup artifacts
Plugins: toolu (register, session-start), statusline (session-start)
Startup notes: <note>; <note>
Startup record: <path>, written <ISO> for <project>
```

The `Startup notes:` line appears only when there are notes. Not ready gives `toolu: not ready — <reason>; every tool call stays denied until OpenCode restarts with the cause fixed`. A missing record gives `toolu: no startup record at <path> — start OpenCode with the toolu plugin in this project, then run the status skill again`. An invalid record gives `toolu: unreadable startup record at <path> — restart OpenCode to rewrite it`.

Generated exclusion reason for `statusline/commands/setup.md`: "Claude Code statusLine setting in settings.json; OpenCode has no statusline setting, so the persistent statusline is host-specific. Use the statusline-status skill."

## Failure modes and edge cases

- **Write failure** (unwritable data root, or a directory at the record path): the verdict stands, one `toolu: status record not written: <reason>` error log entry is sent, and the temp file is removed. The report then shows the previous record or `no startup record`.
- **Not-ready instance:** the record says `not-ready` with the bounded reason. Bash is denied in that session, so the reason reaches the user through the deny message and the log. The record serves a later diagnosis, for example `TOOLU_CONFIG_DIR=<data root> TOOLU_HOST_OVERRIDE=opencode bun …/status.js` from a terminal.
- **Missing, empty, malformed or oversized record** (> 256 KB, wrong `version`, extra keys): the read returns `missing` or `invalid` without throwing, and the report prints the actionable line. Repository, gate and Jev lines still print.
- **Concurrent instances in one project:** both write atomically and the last write wins. Each record is complete and parseable.
- **Status run outside toolu's bash** (no `TOOLU_CONFIG_DIR`): `configRoot` falls back to OpenCode's XDG config directory. The record is normally absent there, so the report says `no startup record`.
- **Duplicate plugin load:** no record and no status entry. The admitted instance writes both.
- **Secrets:** the record and the log `extra` hold only toolu's verdict text, plugin and entry names, counts, paths and startup notes. Env values are never copied in.
- **Protocol output:** the adapter writes nothing to stdout. The status helper's stdout is exactly the report.

## Acceptance criteria

- **AC-1:** With `statusline` and `jev` selected in a real project, OpenCode startup writes a schema-valid `opencode-status.json` under the data root. It lists exactly the selected plugins in startup order, with their entries and artifact counts, the selection source and `status: "ready"`. A not-ready startup, such as a catalog missing a selected plugin's startup bundle, writes `status: "not-ready"` with the bounded bootstrap reason.
- **AC-2:** With `TOOLU_HOST_OVERRIDE=opencode` and `TOOLU_CONFIG_DIR` set to that data root, the statusline `status.js` reports `toolu: ready — N plugins (… selection), M startup artifacts`, a `Plugins:` line with each plugin's entries, and the record path and time. A missing record, an invalid record and a not-ready record each produce their actionable line while the repository, gate and Jev lines still print. Claude and Codex reports are unchanged.
- **AC-3:** The adapter sends exactly one structured `toolu: status` host-log entry per settled startup, with `extra` fields for status, plugins, selection, artifacts and record path. Its fields contain no environment value: a fake secret placed in the env does not appear in the record or the log. The status helper writes only the report to stdout.
- **AC-4:** The generated `statusline-status` skill names OpenCode, runs `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"`, and keeps no `../../` path or Codex label. The setup command stays excluded with the host-specific reason. The surface and contract drift checks pass with `statusline.ui` set to `none` and the host-specific note in place. The statusline README, `docs/statusline/README.md` and `docs/opencode.md` say that the persistent statusline is Claude Code-only on OpenCode and name the status skill and host log as its alternative.
- **AC-5:** On the pinned OpenCode 1.18.34 host, in an isolated profile, a session with `statusline` selected discovers `statusline-status`, runs its command through native bash and returns a report listing the actually selected plugins as ready. `--print-logs` shows the structured status entry, and every nonblank line of the host's `--format json` stdout parses as a JSON event, so toolu adds nothing to the protocol stream. Afterwards, no OpenCode config file in the project or the isolated profile contains `statusLine`, and no `.claude` directory exists in the isolated HOME. With `statusline` unselected, the skill is not discovered.

## Acceptance evidence

| AC | Real input and expected result | Boundary and runnable check |
| --- | --- | --- |
| AC-1 | `createTooluHooks` with the real `prepareEnforcement` over the repo catalog, in a sandbox git project with `.opencode/toolu/plugins.json` `["statusline","jev"]`; read the record through `readOpencodeStatus`. A copied catalog with `plugins/jev/hooks/dist/session-start.js` removed must produce a not-ready record. | `bun test tools/toolu-opencode/src/plugin/__tests__/status-record.test.ts` |
| AC-2 | The real `status.js` bundle spawned against a real git repo, with records produced by the AC-1 writer (ready, not-ready), plus missing, `{}` and truncated-JSON files. Claude and Codex runs of the same repo are compared to their existing expectations. | `bun test plugins/statusline/hooks/src/__tests__/status.test.ts packages/toolu-core/src/startup/__tests__/opencode-status.test.ts` |
| AC-3 | `bindHostContext` with a recording client: the `extra` passes through. The AC-1 run with `TOOLU_FAKE_SECRET=sk-test-359` in env: neither the record bytes nor the logged entries contain it. The status bundle's stdout equals the report and stderr is empty. | `bun test tools/toolu-opencode/src/plugin/__tests__/status-record.test.ts tools/toolu-opencode/src/plugin/__tests__/context.test.ts` |
| AC-4 | The real generator over `plugins/statusline`, plus the committed `generated/` tree, matrix and docs. | `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts tooling/src/__tests__/opencode-host-contract.test.ts`; `bun run check:opencode-surface`; `bun run check:opencode-host`; `grep -l 'Claude Code-only' plugins/statusline/README.md docs/statusline/README.md docs/opencode.md` lists all three files |
| AC-5 | The packed `@toolu/opencode` in an isolated profile, with a scripted provider that calls `skill` then `bash` with the generated command; raw stdout is kept by `runHost` for the protocol check; then a second project without statusline. | `bun run smoke:opencode-entry status.enabled status.disabled` |

## Documentation impact

- `plugins/statusline/README.md` and `docs/statusline/README.md`: an OpenCode section covering the native skill, the toolu readiness lines, the record path, the log entry, and that the persistent statusline is Claude Code-only.
- `plugins/statusline/skills/status/SKILL.md`: the description is unchanged, since it is source text for Codex. The OpenCode text comes from the generator.
- `docs/opencode.md`: a `### Status` subsection.
- `docs/opencode-host-contract.md`: regenerated matrix block, and the generated-catalog sentence names the host-specific setup exclusion.
- `tools/toolu-opencode/README.md`: the catalog sentence says "host-specific" in place of "excluded for OP-25".
- `tools/toolu-opencode/generated/`: regenerated skill, `opencode.toolu.json` and `GENERATED-NOTES.md`.

## Open Questions

None blocking. Decisions taken without a human: (1) the record is written for every settled startup, including when statusline is unselected, so the adapter has no leaf-specific branch and the record stays a generic readiness record; (2) the published `statusline/statusline.sh` renderer helper stays as is (Non-Goal 3), because removing it would change OP-08's startup evidence for no user benefit.
