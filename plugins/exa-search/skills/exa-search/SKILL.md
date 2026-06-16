---
name: exa-search
description: Guides effective use of built-in Exa tools for web search, code search, URL crawling, and deep research. Triggers when searching for external information, looking up docs/APIs, researching technologies, investigating topics, or crawling URLs.
---

# Exa Search

Use this skill when you need to search the web, find code examples, crawl a URL, investigate a topic, or conduct deep research.

**Trigger phrases:** search for, look up, find examples, investigate about, research about, dive into, crawl URL, what is, how does X work

## CLI Tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/exa-search/search.sh" <command> [options]
```

`$CLAUDE_CONFIG_DIR` IS exported into the Bash tool's subshell; `$CLAUDE_PLUGIN_ROOT` is **NOT** — it is only set for hook subprocesses. Using the plugin-root path from a Bash tool call expands to an empty string and runs `/skills/.../search.sh: No such file`. **Always use the published path above.**

Repo-checkout fallback (for tests/dev when the plugin is not installed): `plugins/exa-search/skills/exa-search/scripts/search.sh`.

```
search.sh <command> [options]

Commands:
  search   Search the web (default if no command given)
  crawl    Extract content from URLs via /contents endpoint
  similar  Find pages similar to a URL via /findSimilar endpoint
```

### search (default)

```bash
search.sh search -q "query" [options]
search.sh "query"               # bare query works too

Options:
  -q, --query          Search query (required)
  -n, --num-results    Number of results (default: 10, max: 100)
  -t, --type           instant|fast|auto|deep-lite|deep|deep-reasoning (default: auto)
  -c, --category       company|research paper|news|personal site|financial report|people
  --include-domains    Comma-separated domains to include
  --exclude-domains    Comma-separated domains to exclude
  --start-date         Start published date (YYYY-MM-DD)
  --end-date           End published date (YYYY-MM-DD)
  --include-text       Text that must appear (single phrase, max 5 words)
  --exclude-text       Text to exclude (single phrase, max 5 words)
  --highlights N       Max highlight chars (default: 4000)
  --with-text          Include full text in results
  --lean               Strip image/favicon/subpages/entities — AI-prompt-ready output
```

### crawl

```bash
search.sh crawl <url> [url...] [-m max_chars]

Options:
  -m, --max-chars  Max characters per page (default: 3000)
```

### similar

```bash
search.sh similar <url> [-n num_results] [--highlights N]
```

## Key Constraints

| Constraint | Detail |
|---|---|
| `--include-text` / `--exclude-text` | **Single phrase ONLY.** Multiple values cause 400 errors. |
| `company`/`people` categories | Reject `--start-date`, `--end-date`, `--exclude-domains`. Use other categories when those filters matter. |
| `people` category | `--include-domains` only accepts supported profile domains (LinkedIn et al). |
| `deep-*` types | Adds synthesis latency (~seconds). Use `instant` or `fast` for low-latency lookups. |

## Query Best Practices

1. **Include language context** in code searches: "Go generics" not just "generics"
2. **Use exact identifiers** when available: function names, class names, error messages
3. **Vary query phrasings** for broader coverage: generate 2-3 variations
4. **Use `--with-text`** when you need full page content, not just highlights
5. **Pipe through jq** to extract specific fields: `| jq '.results[] | {title, url}'`
6. **Use `--lean`** when feeding results back to an LLM — strips image/favicon/subpages/entities to keep token count low
