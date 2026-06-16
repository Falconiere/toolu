---
name: context7
description: "Look up library documentation and code examples using Context7. Triggers when the user needs up-to-date docs, API references, or usage examples for any programming library."
---
# Context7 — Library Documentation Lookup
Use this skill to find up-to-date documentation and code examples for any programming library or framework.
## CLI Tool

Invoke at the **stable published path** (a symlink the plugin's SessionStart hook refreshes every session):

```bash
"${CLAUDE_CONFIG_DIR:-$HOME/.claude}/context7/search.sh" <command> [options]
```

`$CLAUDE_CONFIG_DIR` IS exported into the Bash tool's subshell; `$CLAUDE_PLUGIN_ROOT` is **NOT** — it is only set for hook subprocesses. Using the plugin-root path from a Bash tool call expands to an empty string and runs `/skills/.../search.sh: No such file`. **Always use the published path above.**

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
