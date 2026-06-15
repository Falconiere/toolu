# exa-search — Web, Code & URL Search

**Type:** Knowledge | **Version:** 1.18.0 | **Standalone** (no dependencies)

Web search, code-example search, URL crawling, and deep research via the Exa REST API — a skill plus a bash REST wrapper.

## Install

```text
/plugin install exa-search@toolu
```

Set your Exa API key in the environment for the wrapper to authenticate.

## What It Provides

### `exa-search` Skill

Web search, code search, URL crawling, and deep research from the session. Triggers when searching for external information, looking up docs/APIs, investigating technologies, or crawling URLs.

## Usage Examples

### Web Search

```bash
# Quick search (auto picks the right level)
search.sh search -q "Go generics best practices"

# Fast low-latency lookups
search.sh search -q "Rust async trait 2024" -t fast

# Deep research with synthesis
search.sh search -q "WebGPU compute shader architecture" -t deep

# Deep reasoned research
search.sh search -q "How do modern databases handle distributed transactions" -t deep-reasoning

# Limit results
search.sh search -q "CSS container queries" -n 5
```

### Code Search

```bash
# Include language context for better results
search.sh search -q "Go generics higher-order functions" -t fast
search.sh search -q "TypeScript conditional types infer" -t fast
search.sh search -q "Rust async trait object safety" -t fast

# Use exact identifiers when available
search.sh search -q "react useSyncExternalStore example" -t fast
search.sh search -q "tokio select! macro timeout pattern" -t fast
```

### Filtered Search

```bash
# Search only specific domains
search.sh search -q "react server components" --include-domains "nextjs.org,react.dev"

# Exclude domains
search.sh search -q "graphql federation" --exclude-domains "reddit.com"

# Filter by category
search.sh search -q "AI chip startups" -c company
search.sh search -q "transformer attention mechanism" -c "research paper"

# Date range
search.sh search -q "bun runtime" --start-date 2024-01-01 --end-date 2024-12-31

# Text must include or exclude specific terms
search.sh search -q "server actions" --include-text "form" --exclude-text "deprecated"
```

### URL Crawling

```bash
# Crawl a single URL
search.sh crawl https://example.com/docs

# Crawl with custom max characters
search.sh crawl https://docs.example.com -m 5000

# Crawl multiple URLs at once
search.sh crawl https://site1.com https://site2.com/docs
```

### Find Similar Pages

```bash
search.sh similar https://blog.rust-lang.org/2024-async-traits -n 5
```

### Output Control

```bash
# Include full text in results (not just highlights)
search.sh search -q "solidjs signals vs react hooks" --with-text

# Strip images/favicons/entities — lean AI-prompt-ready output
search.sh search -q "bun sqlite benchmarking" --lean

# Pipe through jq to extract specific fields
search.sh search -q "rust memory safety" | jq '.results[] | {title, url}'
```

### Research Workflows

```bash
# Deep investigation with multiple query variations
search.sh search -q "WebGPU vs WebGL performance 2025" -t deep-reasoning
search.sh search -q "webgpu compute shader performance benchmarks" -t deep
search.sh search -q "webgpu rendering pipeline optimization techniques" -t deep

# Crawl results for deeper context
search.sh search -q "wasm runtime comparison" -n 5 --lean
# → pick the top 3 URLs and crawl them
search.sh crawl <url1> <url2> <url3>
```

## Search Types

| Type | Latency | Best For |
|------|---------|----------|
| `instant` | Lowest | Quick fact lookups, identifier checks |
| `fast` | Low | Code examples, API references |
| `auto` | Balanced | Default — Exa chooses the depth |
| `deep-lite` | Medium | Thorough search with lighter synthesis |
| `deep` | High | Synthesized research summaries |
| `deep-reasoning` | Highest | Complex multi-step research |

## Key Constraints

| Constraint | Detail |
|------------|--------|
| `--include-text` / `--exclude-text` | **Single phrase only.** Multiple values cause 400 errors. Max 5 words. |
| `company`/`people` categories | Reject `--start-date`, `--end-date`, `--exclude-domains` |
| `people` category | `--include-domains` only accepts profile domains (LinkedIn, etc.) |
| `deep-*` types | Adds synthesis latency (~seconds). Use `instant`/`fast` for low-latency lookups. |

## Query Best Practices

1. **Include language context** in code searches: `"Go generics"` not just `"generics"`
2. **Use exact identifiers** when available: function names, class names, error messages
3. **Vary query phrasings** for broader coverage: generate 2–3 variations
4. **Use `--with-text`** when you need full page content, not just highlights
5. **Use `--lean`** when feeding results back to an LLM — strips noise to keep token count low

## CLI Reference

```bash
search.sh <command> [options]

Commands:
  search   Search the web (default)
  crawl    Extract content from URLs
  similar  Find pages similar to a URL

### search
search.sh search -q "query" [options]
Options:
  -q, --query          Search query (required)
  -n, --num-results    Number of results (default: 10, max: 100)
  -t, --type           instant|fast|auto|deep-lite|deep|deep-reasoning
  -c, --category       company|research paper|news|personal site|financial report|people
  --include-domains    Comma-separated domains
  --exclude-domains    Comma-separated domains
  --start-date         YYYY-MM-DD
  --end-date           YYYY-MM-DD
  --include-text       Text that must appear (single phrase, max 5 words)
  --exclude-text       Text to exclude (single phrase, max 5 words)
  --highlights N       Max highlight chars (default: 4000)
  --with-text          Include full text
  --lean               Strip images/favicons/subpages

### crawl
search.sh crawl <url> [url...] [-m max_chars]

### similar
search.sh similar <url> [-n num_results] [--highlights N]
```
