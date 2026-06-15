# exa-search

Web, code, and URL search plus deep research via Exa — a skill plus a REST wrapper.

## Install

```
/plugin install exa-search@toolu
```

Standalone, no dependencies.

## What it provides

- **`exa-search` skill** — web search, code-example search, URL crawling, and deep research from the session. Search types range from `instant` / `fast` for low-latency lookups to `deep` / `deep-reasoning` for synthesized research; `crawl` extracts content from one or more URLs.

## The Exa API

The skill drives `scripts/search.sh`, a bash wrapper over the Exa REST API. Set your Exa API key in the environment for the wrapper to authenticate.
