# OpenCode exa-search helper and research workflow — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Codex   **Topic:** Issue #349, native OpenCode use of exa-search

## Problem

OpenCode discovers the generated exa-search skill, but that skill still gives Codex and Claude Code helper paths. The existing SessionStart publishes `search.sh` silently, so the model receives neither its OpenCode path nor a credential-presence fallback. A selected OpenCode plugin must make search, crawl, and similar usable without changing the other hosts' behavior.

## Non-Goals

1. Change Exa's request or response contract, CLI flags, REST retry policy, or credential storage.
2. Add exa-search-specific logic to the shared OpenCode adapter or change the host's permission rules.
3. Claim a live Exa account test from a loopback fixture.

## Architecture

Keep `publishBunCli` in exa-search's SessionStart bundle. On OpenCode only, emit bounded `sessionContext` with the published project helper path, the native skill ID, the three commands, and a branch on `EXA_API_KEY` presence. The selected-plugin bootstrap already delivers this output and removes the owned helper on deselection. Add a targeted `rewriteBody` case for `exa-search-exa-search` so the generated native skill uses `$TOOLU_CONFIG_DIR/exa-search/search.sh`, identifies OpenCode, and gives a useful no-key route. Regenerate the committed surface and bundle. Preserve the source skill for Codex and Claude Code.

## Interfaces / Schema

- Published helper: `<OpenCode project data root>/exa-search/search.sh`, exposed as `$TOOLU_CONFIG_DIR/exa-search/search.sh` in OpenCode bash calls.
- Native skill: `exa-search-exa-search`, loaded through the existing `skills.paths` config contribution.
- Credential: `EXA_API_KEY` is checked for presence in the startup process and read by the CLI from the process environment. No value enters context, logs, or fixture output.
- CLI: `search`, `crawl`, and `similar` retain their current arguments, exit codes, JSON output, and `@toolu/core/rest` transport.

## Failure modes and edge cases

- Missing or empty `EXA_API_KEY`: startup says Exa calls require the key, suggests OpenCode `websearch` when available and `webfetch` for a known URL; the CLI exits 1 before transport with its existing diagnostic.
- Helper publication failure: retain `publishBunCli`'s advisory and omit a misleading ready instruction.
- Disabled exa-search: the existing selection lifecycle removes its owned helper and native skill; no exa startup text reaches the next OpenCode session.
- HTTP errors, non-JSON success, and dropped connections: preserve the existing CLI's status, response, and stderr behavior without leaking the key.

## Acceptance criteria

- **AC-1:** With exa-search selected in an isolated OpenCode profile, native skill discovery loads `exa-search-exa-search`, startup names the executable project helper, and the loaded skill shows OpenCode commands for search, crawl, and similar.
- **AC-2:** The published helper sends each command to its correct Exa endpoint through a loopback HTTPS fixture, preserves successful JSON output and existing HTTP/transport failure behavior, and does not expose `EXA_API_KEY` in model-visible text or diagnostics.
- **AC-3:** With no key, OpenCode startup supplies a usable native fallback and the wrapper makes no Exa request. After deselection, the owned helper, native skill, and startup guidance are absent while core remains ready.
- **AC-4:** Claude Code and Codex SessionStart and skill behavior remain unchanged, and the regenerated OpenCode surface and executable bundles pass drift checks.

## Acceptance evidence

| Criterion | Real input and expected result | Runnable check |
|---|---|
| AC-1 | Pinned OpenCode in an isolated profile selects exa-search; `skill` loads the generated body and the project symlink resolves to the committed bundle. | `bun run smoke:opencode-entry exa.enabled` |
| AC-2 | The published symlink calls `api.exa.ai` through the repository's loopback HTTPS proxy for search, crawl, similar, 401, malformed JSON, and disconnect; no output contains a sentinel key. | `bun test plugins/exa-search/hooks/src/__tests__/search.test.ts` and `bun run smoke:opencode-entry exa.transport` |
| AC-3 | Pinned host starts without a key, then reselects only core; startup fallback appears first and exa-owned contributions disappear second. | `bun run smoke:opencode-entry exa.no-key exa.disabled` |
| AC-4 | Existing-host SessionStart suite, generated surface, bundle, and full repository gate remain green. | `bun test plugins/exa-search/hooks/src/__tests__/session-start.test.ts`; `bun run check:opencode-surface`; `bun run check:plugin-bundles`; `bun run test` |

## Documentation impact

Update the generated OpenCode skill via the generator, and document Exa's OpenCode path, credential fallback, and native commands in `docs/opencode.md`. Keep the source exa-search skill's Codex and Claude Code guidance.

## Open Questions

None. The issue, merged substrate, and selected-plugin precedent settle the scope and ownership.
