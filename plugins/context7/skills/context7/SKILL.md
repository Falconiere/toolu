---
name: context7
description: "ALWAYS ACTIVE — Library documentation protocol. You MUST use the context7 CLI to look up current docs, API references, and usage examples for any third-party library BEFORE answering from memory or searching the web; web search is a fallback only. Triggers on any library/framework API, usage, docs, or version question."
---
# Context7 — Library Documentation Lookup
Use this skill to find up-to-date documentation and code examples for any programming library or framework.
## CLI Tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
# Codex
"${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/context7/search.sh" <command> [options]
# Claude Code
"${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/context7/search.sh" <command> [options]
```

Choose the line for the active host. Ordinary shell calls do not inherit
plugin lifecycle variables, so never collapse these into one ambiguous
fallback. Use the published path; plugin-root variables are lifecycle-only.

Repo-checkout fallback (for tests/dev when the plugin is not installed): `plugins/context7/skills/context7/scripts/search.sh`.
```
search.sh <command> [options]
Commands:
  search  Find libraries by name (resolve library ID)
  docs    Query documentation for a library
```
### Workflow
```
1. search.sh search <library>    → find the library ID
2. search.sh docs <id> <query>   → query its docs
```
### search (default)
```bash
search.sh search <library> [query]
search.sh <library>                 # bare arg works too
Options:
  -l, --library  Library name (required)
  -q, --query    Context for relevance ranking
```
Response includes `id`, `title`, `totalSnippets`, `benchmarkScore` for each match.
### docs
```bash
search.sh docs <library_id> <query>
Options:
  -l, --library-id  Context7 library ID, e.g. /vercel/next.js (required)
  -q, --query       Your question (required)
  -t, --type        json|txt (default: json; txt is plain-text, LLM-prompt-ready)
  --fast            Skip LLM reranking — top vector hits, lower latency, lower relevance
```
JSON response contains `codeSnippets` and `infoSnippets`. Use `-t txt` for plain-text output with code blocks.
Library IDs accept version pinning: `/vercel/next.js/v15.1.8` or `/vercel/next.js@v15.1.8`.
## When to Use
- User asks about a library's API or usage patterns
- Need current docs (beyond training cutoff)
- Looking for code examples with a specific library
- Checking library version compatibility
## Tips
- Be specific in queries: "How to set up JWT auth in Express.js" not "auth"
- Use `-t txt` for readable output you can paste directly
- Pipe JSON through jq: `| jq '.codeSnippets[:3] | .[] | {codeTitle, codeLanguage}'`
- Library IDs use `/org/repo` format — run `search` first to find them
- No API key required (rate-limited). Export `CONTEXT7_API_KEY=ctx7sk...` in the environment for higher limits — the script reads it from the environment only, never from a `.env` file
