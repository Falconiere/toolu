# exa-search

Web, code, and URL search plus deep research via Exa — a skill plus a REST wrapper.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install exa-search@toolu
```

Standalone, no dependencies.

## What it provides

- **`exa-search` skill** — web search, code-example search, URL crawling, and deep research from the session. Search types range from `instant` / `fast` for low-latency lookups to `deep` / `deep-reasoning` for synthesized research; `crawl` extracts content from one or more URLs.

## The Exa API

The skill drives a TypeScript CLI over the Exa REST API, shipped as the executable Bun bundle `hooks/dist/search.js` (source `hooks/src/search.ts`) and published at `<config>/exa-search/search.sh` by a SessionStart hook. Set `EXA_API_KEY` in the environment for it to authenticate; it never reads a `.env` file. Exit status: `1` usage, missing key or a failed connection, `2` a number flag jq would reject, `22` HTTP error (the error body on stdout, also under `--lean`), `5` non-JSON response, `141` when the stdout reader closes early. Redirects are not followed, so the key never reaches another host.
