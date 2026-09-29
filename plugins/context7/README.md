# context7

Library documentation & code-example lookup via Context7 — a skill plus a REST wrapper.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install context7@toolu
```

Standalone, no dependencies.

## What it provides

- **`context7` skill** — finds up-to-date documentation and code examples for any programming library or framework: `search` to resolve a library ID, then fetch its docs. Triggers when you need current docs, API references, or usage examples.

## The Context7 API

The skill drives a TypeScript CLI over the Context7 REST API, shipped as the executable Bun bundle `hooks/dist/search.js` (source `hooks/src/search.ts`) and published at `<config>/context7/search.sh` by a SessionStart hook. Exit status: `1` usage or a failed connection, `22` HTTP error (the error body on stdout, raw for `-t txt`), `5` non-JSON response, `141` when the stdout reader closes early. Redirects are not followed. **No API key required** (rate-limited). For higher limits, export `CONTEXT7_API_KEY=ctx7sk...` in the environment — the script reads it from the environment only, never from a `.env` file.
