/**
 * `--lean`: the bash jq projection, kept field for field —
 *   {requestId, results: [(.results // [])[] | {title, url, publishedDate,
 *    author, highlights, text, summary} | with_entries(select(.value != null
 *    and .value != ""))]}
 * requestId stays even when null; empty and null result fields are dropped.
 */
const RESULT_FIELDS = ["title", "url", "publishedDate", "author", "highlights", "text", "summary"];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function leanResult(result: unknown): Record<string, unknown> {
  const source = isRecord(result) ? result : {};
  const kept: Record<string, unknown> = {};
  for (const field of RESULT_FIELDS) {
    const value = source[field];
    if (value !== undefined && value !== null && value !== "") kept[field] = value;
  }
  return kept;
}

export function leanResponse(response: unknown): Record<string, unknown> {
  const source = isRecord(response) ? response : {};
  const results = Array.isArray(source["results"]) ? source["results"] : [];
  return { requestId: source["requestId"] ?? null, results: results.map(leanResult) };
}
