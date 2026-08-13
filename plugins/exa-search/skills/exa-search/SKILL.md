---
name: exa-search
description: ALWAYS ACTIVE — Web search/crawl protocol. You MUST use the exa-search CLI (search / crawl / similar) instead of host-native web tools for web searches, code-example hunts, URL crawling, and topic research whenever EXA_API_KEY is set; native tools are a fallback only. Triggers when searching for external information, looking up docs/APIs, researching technologies, investigating topics, or crawling URLs.
---

# Exa Search

Use this skill when you need to search the web, find code examples, crawl a URL, investigate a topic, or conduct deep research.

**Trigger phrases:** search for, look up, find examples, investigate about, research about, dive into, crawl URL, what is, how does X work

## CLI Tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
# Codex
"${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/exa-search/search.sh" <command> [options]
# Claude Code
"${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/exa-search/search.sh" <command> [options]
```

Choose the line for the active host. Ordinary shell calls do not inherit
plugin lifecycle variables, so never collapse these into one ambiguous
fallback. Use the published path; plugin-root variables are lifecycle-only.

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
