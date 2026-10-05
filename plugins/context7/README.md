# context7

> **Deprecated:** context7 will be removed in v8.0.0. Use your host's native web search and fetch tools instead. Uninstall it with `claude plugin uninstall context7@toolu` (Claude Code), `codex plugin remove context7@toolu` (Codex) or `npx @toolu/plugins remove context7 --host opencode --yes` (OpenCode). Until then it keeps working and shows a one-line notice when a session starts.

Library documentation & code-example lookup via Context7 — a skill plus a REST wrapper.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install context7@toolu
```

Standalone, no dependencies.

OpenCode: add `context7` to `.opencode/toolu/plugins.json` (see [OpenCode install](../../docs/opencode.md)).

- The helper lives at `$TOOLU_CONFIG_DIR/context7/search.sh`, the per-project data root exported to every bash call.
- The skill is `context7-context7`.
- context7's own SessionStart gives the documentation-first instruction, so it appears only while context7 is selected; toolu's session protocol omits its context7 line on OpenCode. The instruction rides on every model request and is not repeated in compaction context.
- The instruction's command is `'<bun>' --no-env-file '<helper>'` and the skill's is `"$TOOLU_BUN" --no-env-file "…/context7/search.sh"`. Bun is named by path, so `bun` need not be on `PATH`, and `--no-env-file` keeps a project `.env` from supplying `CONTEXT7_API_KEY`. A file of your own at the helper path is kept and named alone.

## What it provides

- **`context7` skill** — finds up-to-date documentation and code examples for any programming library or framework: `search` to resolve a library ID, then fetch its docs. Triggers when you need current docs, API references, or usage examples.

## The Context7 API

The skill drives a TypeScript CLI over the Context7 REST API, shipped as the executable Bun bundle `hooks/dist/search.js` (source `hooks/src/search.ts`) and published at `<config>/context7/search.sh` by a SessionStart hook. Exit status: `1` usage or a failed connection, `22` HTTP error (the error body on stdout, raw for `-t txt`), `5` non-JSON response, `141` when the stdout reader closes early. Redirects are not followed. **No API key required** (rate-limited). For higher limits, export `CONTEXT7_API_KEY=ctx7sk...` in the environment — the script reads it from the environment only, never from a `.env` file.
