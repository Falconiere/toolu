# context7 — Live Library Documentation Lookup

**Type:** Knowledge | **Version:** 4.5.0 | **Standalone** (no dependencies)

Library documentation and code-example lookup via the Context7 REST API — a skill plus a bash REST wrapper.

## Install

```text
/plugin install context7@toolu
```

**No API key required** (rate-limited). For higher limits, export `CONTEXT7_API_KEY=ctx7sk...` in the environment — the script reads it from the environment only, never from a `.env` file.

## What It Provides

### `context7` Skill

Finds up-to-date documentation and code examples for any programming library or framework. Triggers when the user needs current docs, API references, or usage examples.

## Usage Examples

### Find a Library

```bash
# Search for a library by name to get its ID
search.sh search next.js
search.sh search react
search.sh search "express auth middleware"

# Bare arg also works
search.sh "tokio runtime"
```

Response includes `id`, `title`, `totalSnippets`, and `benchmarkScore` for each match.

### Query Documentation

```bash
# Once you have the library ID, query its docs
search.sh docs /vercel/next.js "How to set up middleware"
search.sh docs /facebook/react "useEffect cleanup function"
search.sh docs /tokio-rs/tokio "spawn vs spawn_blocking"

# Version pinning
search.sh docs /vercel/next.js/v15.1.8 "app router vs pages router"
search.sh docs /vercel/next.js@v15.1.8 "middleware API"

# Skip LLM reranking for faster results
search.sh docs /vercel/next.js "server actions" --fast
```

### Output Formats

```bash
# JSON (default) — structured with codeSnippets and infoSnippets
search.sh docs /vercel/next.js "image component"

# Plain text — LLM-prompt-ready, with code blocks
search.sh docs /vercel/next.js "image component" -t txt

# Pipe JSON through jq to extract specific fields
search.sh docs /vercel/next.js "layout" | jq '.codeSnippets[:3] | .[] | {codeTitle, codeLanguage}'
```

### Complete Workflow

```bash
# Step 1: Search for the library
search.sh search prisma
# → [{"id": "/prisma/prisma", "title": "Prisma", ...}]

# Step 2: Query specific docs
search.sh docs /prisma/prisma "How to create a many-to-many relation" -t txt

# Step 3: Need a specific version?
search.sh docs /prisma/prisma/v5.0.0 "upsert with nested create" -t txt

# Step 4: Quick lookup — skip reranking
search.sh docs /prisma/prisma "transaction with interactive API" --fast -t txt
```

### When to Use

- User asks about a library's API or usage patterns
- Need current docs beyond the training cutoff date
- Looking for code examples with a specific library
- Checking library version compatibility or migration guides

### Query Tips

- **Be specific**: "How to set up JWT auth in Express.js" not just "auth"
- **Use `-t txt`** for readable output you can paste directly into context
- **Use `--fast`** when you need quick top-N hits, not reranked precision
- **Version-pin** for accuracy: `/vercel/next.js/v15.1.8`
- Library IDs use `/org/repo` format — always run `search` first to find them

## CLI Reference

```bash
search.sh <command> [options]

Commands:
  search  Find libraries by name (resolve library ID)
  docs    Query documentation for a library

### search
search.sh search <library> [query]
Options:
  -l, --library  Library name (required)
  -q, --query    Context for relevance ranking

### docs
search.sh docs <library_id> <query>
Options:
  -l, --library-id  Context7 library ID (required)
  -q, --query       Your question (required)
  -t, --type        json|txt (default: json)
  --fast            Skip LLM reranking
```
