/** Help text, byte-for-byte what the bash search.sh printed to stderr. */

export const MAIN_USAGE = `Exa Search CLI

Usage: search.sh <command> [options]

Environment:
  EXA_API_KEY  Required. Exa API key.

Commands:
  search   Search the web (default if no command given)
  crawl    Extract content from URLs
  similar  Find pages similar to a URL

Run 'search.sh <command>' with no args for command-specific help.`;

export const SEARCH_USAGE = `Usage: search.sh search -q <query> [options]
  -n, --num-results  Number of results (default: 10)
  -t, --type         instant|fast|auto|deep-lite|deep|deep-reasoning (default: auto)
  -c, --category     company|research paper|news|personal site|financial report|people
  --include-domains  Comma-separated domains to include
  --exclude-domains  Comma-separated domains to exclude
  --start-date       Start published date (YYYY-MM-DD)
  --end-date         End published date (YYYY-MM-DD)
  --include-text     Text that must appear in results
  --exclude-text     Text to exclude from results
  --highlights N     Max highlight chars (default: 4000)
  --with-text        Include full text in results
  --lean             Strip image/favicon/subpages/entities for AI prompts`;

export const CRAWL_USAGE = `Usage: search.sh crawl <url> [url...] [-m max_chars]
  Extracts content from one or more URLs
  -m, --max-chars  Max characters per page (default: 3000)`;

export const SIMILAR_USAGE = `Usage: search.sh similar <url> [-n num_results]
  Finds pages similar to the given URL
  -n, --num-results  Number of results (default: 10)
  --highlights N     Max highlight chars (default: 4000)`;
