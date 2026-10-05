# Deprecation notice for exa-search, context7, jira and agent-browser — Design

**Date:** 2026-10-05   **Status:** Draft   **Author:** Claude (epic worker, #403)   **Topic:** One release of notice before #406 removes four plugins

## Problem

Epic #402 removes exa-search, context7, jira and agent-browser instead of porting them (#406, a `feat!` commit). Claude Code and Codex keep a cached copy of a plugin after the marketplace drops it, and its hooks keep running, so users must uninstall these plugins themselves. They need one release that tells them how, on the host they use, before the removal lands. Nothing about the plugins' behaviour changes in this release.

## Non-Goals

1. No behaviour change: publishing, credentials, guidance text and CLIs stay as they are.
2. The Bun-missing fallback in each `hooks.json` launcher stays as it is. Without Bun the hook never runs, and the launcher's own `systemMessage` already replaces it. The launcher is generated and gated by `check:hooks-json`.
3. Cursor and Hermes get no host-specific uninstall command. toolu has no plugin manager command for them (`INSTALL` is `null` there too).
4. No removal here, and no change to `docs/plugins/index.md`, the marketplace files or the OpenCode capability matrix. All of that belongs to #406.

## Architecture

- **One shared helper in `@toolu/core`.** `host-roots.ts` gets an `UNINSTALL` table beside `INSTALL` and `pluginUninstallCommand(name, options)`. A new `startup/deprecation.ts` builds the line and the hook's stdout from that table. Each of the four `session-start.ts` entries calls it once, so the wording, the version and the host commands live in one place.
- **Line format:** `<plugin> is deprecated and will be removed in v8.0.0; uninstall with: <command>`. This is the issue's scenario text, with the version resolved.
  - **Why v8.0.0:** #406 is `feat!`, and release-please turns that into the next major from 7.x. The issue's `vX.Y+1.0` is a template, and following it literally (v7.11.0) would contradict the release rules. Jev chose v8 at 0.93.
  - The constant is `REMOVAL_RELEASE`, exported once.
- **Channel:** a top-level `systemMessage`, never `additionalContext`, so it costs no model context.
  - Claude Code and Codex show it to the user. `docs/runtime.md` records that Codex's SessionStart output schema accepts it, checked against `tooling/fixtures/codex-hook-schemas/`.
  - On OpenCode, the bootstrap's `parseStartupOutput` already accepts `systemMessage` next to `hookSpecificOutput`. `deliveryPlan` (`tools/toolu-opencode/src/plugin/enforcement.ts`) turns it into one `toolu: <plugin>/session-start: <line>` info log entry when the host loads. That is the OpenCode startup channel. Tool calls never re-run SessionStart.
- **One JSON object.**
  - On Claude Code and Codex these hooks are silent on success, so stdout becomes `{"systemMessage":"…"}`.
  - On OpenCode, exa-search, context7, jira and agent-browser already print `hookSpecificOutput`. The notice is added as a sibling key in that same object (`renderHookOutput({...context, systemMessage}, false)`).
- **OpenCode compaction:** OpenCode re-runs SessionStart with `source: "compact"` at each compaction (`contextJobs`), and `context-delivery.ts` would log the `systemMessage` again.
  - On OpenCode the notice is skipped when the stdin source is `compact`, so it appears once per host start. context7 and jira already read that source through `compacting()`, and exa-search and agent-browser reuse the same check. Jev answered 0.88 for suppressing it.
  - On Claude Code and Codex every SessionStart invocation prints it once, compaction included. Each one is a session start the user sees.
- **Reuse:** `publishBunCli` and `publishWrapper`, `renderHookOutput` and `sessionContext` (`@toolu/core/startup`), `detectHost` (`@toolu/core/host`), the `publishedCliSuite` conformance harness, and the `createTooluHooks` hermetic OpenCode fixtures (`jev-fixtures.ts`).

## Interfaces / Schema

```ts
// packages/toolu-core/src/host/host-roots.ts
const UNINSTALL: Readonly<Record<HostName, ((name: string) => string) | null>> = {
  claude: (name) => `claude plugin uninstall ${name}@toolu`,
  codex: (name) => `codex plugin remove ${name}@toolu`,
  opencode: (name) => `npx @toolu/plugins remove ${name} --host opencode --yes`,
  cursor: null,
  hermes: null,
};
export function pluginUninstallCommand(name: string, options?: HostOptions): string | null;

// packages/toolu-core/src/startup/deprecation.ts (re-exported from startup.ts)
export const REMOVAL_RELEASE = "v8.0.0";
/** "<plugin> is deprecated and will be removed in v8.0.0; uninstall with: <command>" */
export function deprecationNotice(plugin: string, options?: HostOptions): string;
/** The hook's whole stdout: `context` (may be undefined) plus the notice as `systemMessage`. */
export function deprecatedStartupOutput(
  plugin: string,
  context: SessionContext | undefined,
  options?: HostOptions,
): string;
```

Host commands (Claude Code's `claude plugin uninstall` and Codex's `codex plugin remove` are the argv `tools/toolu-cli/src/host/{claude,codex}.ts` already runs; OpenCode's is documented in `docs/opencode.md#update-roll-back-and-remove`):

| Host | Line for context7 |
|---|---|
| Claude Code | `context7 is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall context7@toolu` |
| Codex | `context7 is deprecated and will be removed in v8.0.0; uninstall with: codex plugin remove context7@toolu` |
| OpenCode | `context7 is deprecated and will be removed in v8.0.0; uninstall with: npx @toolu/plugins remove context7 --host opencode --yes` |
| Cursor, Hermes | `context7 is deprecated and will be removed in v8.0.0; uninstall it with your host's plugin manager` |

README and docs banner, the first block after the H1:

```markdown
> **Deprecated:** <plugin> will be removed in v8.0.0. <Replacement sentence> Uninstall it with `claude plugin uninstall <plugin>@toolu` (Claude Code), `codex plugin remove <plugin>@toolu` (Codex) or `npx @toolu/plugins remove <plugin> --host opencode --yes` (OpenCode).
```

Replacement sentences:
- **exa-search and context7:** "Use your host's native web search and fetch tools instead."
- **jira:** "For Jira epics, use epic-orchestrator's built-in Jira tracker instead."
- **agent-browser:** "It has no replacement."

## Failure modes and edge cases

- **Publish fails or the bundle is missing** (`source-missing`, `unwritable`, `link-failed`): the notice is still printed once. It does not depend on publishing. stderr lines such as the cannot-create line and the Bun-off-PATH advisory are unchanged.
- **Unreadable or empty stdin:** this counts as a plain start (`compacting()` returns false), so the notice is printed.
- **OpenCode with `source: "compact"`:** no `systemMessage`. context7's and jira's existing silence on compaction stays. exa-search and agent-browser keep printing their unchanged startup context, without the notice.
- **Invalid `TOOLU_HOST_OVERRIDE`:** `detectHost` warns on stderr and falls back to environment detection, as it does today. The line uses the detected host's command.
- **Cursor and Hermes:** the line ends with the generic sentence. It never prints a command that does not exist.
- **No Bun at all:** the launcher's Bun-missing `systemMessage` is the only output, unchanged (Non-Goal 2).
- **OpenCode output validation:** `{hookSpecificOutput, systemMessage}` and `{systemMessage}` both pass the strict `StartupOutput` schema. Any other key would fail closed and is never added.

## Acceptance criteria

- **AC-1:** Given each of the four plugins on Claude Code (`startupEnv("claude")`, stdin `{"source":"startup"}`), the real `hooks.json` launcher exits 0. Its stdout is exactly `{"systemMessage":"<plugin> is deprecated and will be removed in v8.0.0; uninstall with: claude plugin uninstall <plugin>@toolu"}` plus a newline, and its stderr and the published helper are unchanged.
- **AC-2:** Given the same run on Codex (`startupEnv("codex")`), the stdout is the same object with `codex plugin remove <plugin>@toolu`.
- **AC-3:** Given OpenCode with `toolu` and all four plugins enabled, `createTooluHooks` logs exactly one `<plugin> is deprecated … npx @toolu/plugins remove <plugin> --host opencode --yes` line per plugin at load. Tool calls and a compaction add no further notice lines. The existing OpenCode `additionalContext` of each plugin is unchanged.
- **AC-4:** `pluginUninstallCommand` returns the Claude Code, Codex and OpenCode commands above and `null` for Cursor and Hermes. `deprecationNotice` uses the generic sentence for Cursor and Hermes.
- **AC-5:** The notice is never `additionalContext`. On Claude Code and Codex the stdout object has no `hookSpecificOutput` key. On OpenCode the notice text never appears in `additionalContext`.
- **AC-6:** The four plugin READMEs, `docs/{exa-search,context7,jira}/README.md` and `plugins/agent-browser/skills/agent-browser/SKILL.md` (with its generated OpenCode copy) carry the banner naming v8.0.0 and the replacement. `check:opencode-surface` and the docs-sync gate pass.
- **AC-7:** `bun run test` is green.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1, AC-2, AC-5 | Each plugin's real `hooks/hooks.json` launcher and built `hooks/dist/session-start.js` in a sandbox | Exact stdout per host. The publish, idempotence, Bun-off-PATH advisory and missing-bundle cases still pass, now with the notice on stdout | Missing bundle and Bun off PATH still print the notice | `bun test plugins/{exa-search,context7,jira,agent-browser}/hooks/src/__tests__/session-start.test.ts` (shared `publishedCliSuite` with a per-host `notice` spec) |
| AC-3, AC-5 | The real toolu and plugin bundles through `createTooluHooks`, a project selecting all four | One log line per plugin, none added by `tool.execute.before`, `tool.execute.after` or a compaction | Compaction (`source: "compact"`) | `bun test tools/toolu-opencode/src/plugin/__tests__/deprecation-notice.test.ts` |
| AC-3 | exa-search and agent-browser on OpenCode, as in the existing per-plugin tests | `hookSpecificOutput` unchanged plus `systemMessage` | No key, and compact source | `bun test plugins/{exa-search,agent-browser}/hooks/src/__tests__/session-start.test.ts` |
| AC-4 | Every host name | Exact command or `null` | Cursor and Hermes | `bun test packages/toolu-core/src/startup/__tests__/deprecation.test.ts` |
| AC-6 | The edited Markdown files | Banner present. Generated surface matches | Generated drift | `bun run check:opencode-surface` and `bun test packages/toolu-core/src/startup/__tests__/deprecation.test.ts` (banner check over the eight files) |
| AC-7 | Whole repository | Green | — | `bun run test` |

## Documentation impact

- **Deprecation banners:** the four plugin READMEs, `docs/exa-search/README.md`, `docs/context7/README.md`, `docs/jira/README.md`, and `plugins/agent-browser/skills/agent-browser/SKILL.md` with its regenerated OpenCode copy.
- **`docs/runtime.md` (Startup hooks paragraph):** add one sentence saying these four hooks print the deprecation `systemMessage`.
- **CHANGELOG:** release-please writes the entry from the `feat(deprecation):` commit title.

## Open Questions

None blocking.
- **Removal version:** v8.0.0, decided above. If another `feat!` lands before #406, #406 updates `REMOVAL_RELEASE`. Owner: #406.
- **Jira replacement:** the banner names epic-orchestrator's built-in Jira tracker (#404), which ships in parallel. If #404 changes its name, #406 updates the banner when it deletes these files.
