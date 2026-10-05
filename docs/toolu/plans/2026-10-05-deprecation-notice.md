# Deprecation notice for exa-search, context7, jira and agent-browser — Plan

**Date:** 2026-10-05   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-05-deprecation-notice-design.md   **Topic:** One systemMessage notice per session start, and banners, before #406 removes four plugins

## Evidence and approach

**Inspected:**
- `packages/toolu-core/src/host/host-roots.ts`: the `INSTALL` table and `pluginInstallCommand`.
- `packages/toolu-core/src/startup/{context,publish,dependencies}.ts`.
- The four `plugins/*/hooks/src/session-start.ts` entries. All are silent on Claude Code and Codex, and print `hookSpecificOutput` on OpenCode.
- `tools/toolu-conformance/src/harness/startup.ts`: `publishedCliSuite` asserts `stdout: ""`.
- `tools/toolu-opencode/src/bootstrap/{output,runtime}.ts`: SessionStart stdin is `{"source":"startup"}`, and `systemMessage` is accepted.
- `tools/toolu-opencode/src/plugin/{enforcement,hooks,context-delivery}.ts`: one notice log line at load, and compact jobs log a `systemMessage` again.
- `tools/toolu-cli/src/host/{claude,codex}.ts`: the uninstall argv.
- `docs/opencode.md#update-roll-back-and-remove`: the OpenCode `remove` command.

**Approach:**
- Add a core uninstall table and a deprecation helper. Each plugin's SessionStart prints one JSON object carrying the notice as `systemMessage`. On OpenCode the notice is skipped when the hook runs for compaction.
- Extend the shared conformance harness with an exact per-host notice.
- Add a hermetic OpenCode test through `createTooluHooks`.
- Add banners in Markdown, regenerate the OpenCode surface, and add one sentence to `docs/runtime.md`.

## Workstream summary

core helper → plugin entries + harness + bundles → OpenCode once-at-load proof → banners, docs and generated surface → full gate → feat commit title.

## Steps (machine-readable)

```json
[
  {
    "id": "core-helper",
    "title": "Add UNINSTALL/pluginUninstallCommand to host-roots, startup/deprecation.ts (REMOVAL_RELEASE, deprecationNotice, deprecatedStartupOutput, startedByCompaction), export them, with colocated tests",
    "ac_refs": ["AC-4", "AC-5"],
    "paths": ["packages/toolu-core/src/host/**", "packages/toolu-core/src/startup/**", "packages/toolu-core/package.json"],
    "input": "Every HostName via explicit host option and via real env detection (TOOLU_HOST_OVERRIDE=opencode, PLUGIN_ROOT set, CURSOR_VERSION set); a SessionContext and undefined; compacting true/false; stdin strings '{\"source\":\"compact\"}', '{\"source\":\"startup\"}', '', 'not json'",
    "check": "bun test packages/toolu-core/src/startup/__tests__/deprecation.test.ts packages/toolu-core/src/host/__tests__",
    "model": "inherit"
  },
  {
    "id": "plugin-entries",
    "title": "Print the notice from the four session-start entries (OpenCode compaction skipped), add an exact per-host notice to publishedCliSuite, assert it per plugin on Claude and Codex and on OpenCode start/compact, rebuild bundles",
    "ac_refs": ["AC-1", "AC-2", "AC-5"],
    "depends_on": ["core-helper"],
    "paths": ["plugins/exa-search/hooks/**", "plugins/context7/hooks/**", "plugins/jira/hooks/**", "plugins/agent-browser/hooks/**", "plugins/toolu-review/**", "tools/toolu-conformance/src/harness/**", "packages/toolu-core/src/**", "tooling/src/build-plugins.ts"],
    "input": "Each plugin's real hooks.json launcher and rebuilt hooks/dist/session-start.js in a sandbox with Claude and Codex env (stdin {\"source\":\"startup\"}), bun off PATH, a missing CLI bundle, and OpenCode env with stdin source startup and compact",
    "check": "bun run check:plugin-bundles && bun test --timeout 60000 plugins/exa-search/hooks/src/__tests__/session-start.test.ts plugins/context7/hooks/src/__tests__/session-start.test.ts plugins/jira/hooks/src/__tests__/session-start.test.ts plugins/agent-browser/hooks/src/__tests__/session-start.test.ts plugins/toolu-review/hooks/src/__tests__/session-start.test.ts plugins/context7/hooks/src/__tests__/opencode.test.ts plugins/jira/hooks/src/__tests__/opencode.test.ts",
    "model": "inherit"
  },
  {
    "id": "opencode-once",
    "title": "Prove on the OpenCode adapter that each of the four notices is logged exactly once at load and not again on tool calls or compaction",
    "ac_refs": ["AC-3", "AC-5"],
    "depends_on": ["plugin-entries"],
    "paths": ["tools/toolu-opencode/src/**", "plugins/exa-search/**", "plugins/context7/**", "plugins/jira/**", "plugins/agent-browser/**", "plugins/toolu/**", "packages/toolu-core/src/**"],
    "input": "createTooluHooks with the real prepareEnforcement over the repo catalog in a sandbox git project selecting toolu, exa-search, context7, jira and agent-browser; a recording log client; a bash tool.execute.before/after and an experimental.session.compacting call",
    "check": "bun test --timeout 120000 tools/toolu-opencode/src/plugin/__tests__/deprecation-notice.test.ts tools/toolu-opencode/src/plugin/__tests__/context7-delivery.test.ts tools/toolu-opencode/src/plugin/__tests__/jira-delivery.test.ts",
    "model": "inherit"
  },
  {
    "id": "docs-banners",
    "title": "Add the deprecation banner to the four plugin READMEs, docs/{exa-search,context7,jira}/README.md and agent-browser's SKILL.md, regenerate the OpenCode surface, note the notice in docs/runtime.md, and gate the banners with a tooling test",
    "ac_refs": ["AC-6"],
    "depends_on": ["core-helper"],
    "paths": ["plugins/exa-search/README.md", "plugins/context7/README.md", "plugins/jira/README.md", "plugins/agent-browser/README.md", "plugins/agent-browser/skills/**", "docs/exa-search/**", "docs/context7/**", "docs/jira/**", "docs/runtime.md", "tools/toolu-opencode/generated/**", "tools/toolu-opencode/scripts/**", "tooling/src/__tests__/deprecation-banners.test.ts", "packages/toolu-core/src/startup/deprecation.ts", "plugins/*/skills/**", "plugins/toolu/scripts/context-budget.ts"],
    "input": "The eight committed Markdown files and the generated agent-browser-agent-browser skill",
    "check": "bun run check:opencode-surface && bun run test:context-budget && bun test tooling/src/__tests__/deprecation-banners.test.ts",
    "model": "inherit"
  },
  {
    "id": "full-gate",
    "title": "Run the full repository gate",
    "ac_refs": ["AC-7"],
    "depends_on": ["plugin-entries", "opencode-once", "docs-banners"],
    "check": "bun run test",
    "model": "inherit"
  },
  {
    "id": "feat-title",
    "title": "Land the implementation as a feat(deprecation): commit so release-please lists it",
    "ac_refs": ["AC-8"],
    "depends_on": ["full-gate"],
    "check": "git log origin/main..HEAD --format=%s | grep -q '^feat(deprecation): '",
    "model": "inherit"
  }
]
```

## Deviations

- `plugin-entries` no longer runs jev's `session-start.test.ts`. Its only failure on this host is the root-only "refuses the link" chmod case. Comemory be52369e records that it also fails on origin/main as root. The jev suite passed every other case here (50 pass, 1 fail), and CI runs it in full through `bun run test`.

- The full gate showed that `plugins/{context7,jira}/hooks/src/__tests__/opencode.test.ts` parse OpenCode startup stdout with a strict schema. The schema now expects the exact notice, and `plugin-entries` runs both files.

## Critical files

- `packages/toolu-core/src/host/host-roots.ts`, `packages/toolu-core/src/host/host.ts` (export)
- `packages/toolu-core/src/startup/deprecation.ts` (new), `packages/toolu-core/src/startup/startup.ts`, `packages/toolu-core/src/startup/__tests__/deprecation.test.ts` (new)
- `plugins/{exa-search,context7,jira,agent-browser}/hooks/src/session-start.ts`, their `hooks/dist/session-start.js` and `__tests__/session-start.test.ts`
- `tools/toolu-conformance/src/harness/startup.ts`
- `tools/toolu-opencode/src/plugin/__tests__/deprecation-notice.test.ts` (new)
- `plugins/{exa-search,context7,jira,agent-browser}/README.md`, `docs/{exa-search,context7,jira}/README.md`, `plugins/agent-browser/skills/agent-browser/SKILL.md`, `tools/toolu-opencode/generated/skills/agent-browser-agent-browser/SKILL.md`, `docs/runtime.md`
- `tooling/src/__tests__/deprecation-banners.test.ts` (new)

## Verification

- **End to end:** the real launchers print the exact per-host line on Claude Code and Codex, and the real OpenCode adapter logs one line per plugin at load and none on tool calls or compaction. Both run on real bundles in sandboxes, with no mocks.
- **Boundaries:**
  - a missing CLI bundle and Bun off PATH still print the notice;
  - an OpenCode compaction prints none;
  - Cursor and Hermes get the generic sentence;
  - unreadable stdin counts as a plain start.
- **Documentation:** the banners are checked by a tooling test, the generated surface by `check:opencode-surface`, and `docs/runtime.md` is updated. The docs-sync push gate sees doc changes.
- **Final:** `bun run test` through the job wrapper, then `plan-ledger.js run <plan> --verify`.
