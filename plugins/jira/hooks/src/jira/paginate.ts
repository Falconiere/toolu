/**
 * Jira's two pagination models, concatenating every page's items:
 *   token  — v3 /search/jql: nextPageToken until isLast, no total.
 *   offset — v2 /search: startAt/maxResults until total is reached.
 * Both POST the body with the paging fields merged in. A page that fails
 * exits 1 with nothing on stdout, as bash's `page=$(…) || return 1` did.
 */
import { CliExit } from "@toolu/core/cli";
import { type Conn, lookup } from "./http.ts";
import { alt, get, type JsonObject } from "./jq.ts";

/** Bash's loop guard. */
const MAX_PAGES = 1000;

function items(page: unknown, key: string): unknown[] {
  const found = alt(get(page, key), []);
  if (!Array.isArray(found)) throw new CliExit(5, `jq: error: cannot add ${typeof found} to array`);
  return found;
}

export async function paginate(
  conn: Conn,
  mode: "token" | "offset",
  path: string,
  body: JsonObject,
  key: string,
): Promise<unknown[]> {
  const max = alt(body["maxResults"], 50);
  let all: unknown[] = [];
  let token = "";
  let start = 0;
  for (let guard = 0; guard < MAX_PAGES; guard += 1) {
    // Merged like jq's `. + {…}`: an existing key keeps its place, a new one is appended.
    let request: JsonObject = { ...body, startAt: start, maxResults: max };
    if (mode === "token") request = token === "" ? body : { ...body, nextPageToken: token };
    const page = await lookup(conn, path, { method: "POST", body: request });
    const got = items(page, key);
    all = [...all, ...got];
    if (mode === "token") {
      const next = alt(get(page, "nextPageToken"), "");
      token = typeof next === "string" ? next : JSON.stringify(next);
      if (get(page, "isLast") === true || token === "") break;
    } else {
      if (got.length === 0) break;
      start += got.length;
      if (start >= Number(alt(get(page, "total"), 0))) break;
    }
  }
  return all;
}
