/** Help text, byte-for-byte what the bash search.sh printed to stderr. */

export const MAIN_USAGE = `Context7 CLI — Library Documentation Lookup

Usage: search.sh <command> [options]

Environment:
  CONTEXT7_API_KEY  Optional. If set and starts with 'ctx7sk', sent as Bearer token.

Commands:
  search  Find libraries by name (resolve library ID)
  docs    Query documentation for a library

Workflow:
  1. search.sh search <library>    # find the library ID
  2. search.sh docs <id> <query>   # query its docs

Run 'search.sh <command>' with no args for command-specific help.`;

export const SEARCH_USAGE = `Usage: search.sh search <library> [query]
  Searches for libraries matching the name

  -l, --library  Library name (required)
  -q, --query    Context for ranking results

Examples:
  search.sh search react
  search.sh search tokio "async runtime for Rust"`;

export const DOCS_USAGE = `Usage: search.sh docs <library_id> <query>
  Retrieves documentation context for a library

  -l, --library-id  Context7 library ID, e.g. /vercel/next.js (required)
  -q, --query       Your question (required)
  -t, --type        Output format: json|txt (default: json; txt is LLM-prompt-ready)
  --fast            Skip LLM reranking, return top vector-search hits (lower latency)

Examples:
  search.sh docs /vercel/next.js "app router file conventions"
  search.sh docs /tokio-rs/tokio "spawn async tasks" -t txt

Tip: Run 'search.sh search <name>' first to find the library ID.`;
