# context7

Library documentation & code-example lookup via Context7 — a skill plus a REST wrapper.

## Install

```
/plugin install context7@toolu
```

Standalone, no dependencies.

## What it provides

- **`context7` skill** — finds up-to-date documentation and code examples for any programming library or framework: `search` to resolve a library ID, then fetch its docs. Triggers when you need current docs, API references, or usage examples.

## The Context7 API

The skill drives `scripts/search.sh`, a bash wrapper over the Context7 REST API. **No API key required** (rate-limited). For higher limits, export `CONTEXT7_API_KEY=ctx7sk...` in the environment — the script reads it from the environment only, never from a `.env` file.
