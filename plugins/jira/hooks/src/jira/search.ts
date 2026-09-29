/**
 * JQL search. v3 (Cloud) POSTs to /rest/api/3/search/jql with nextPageToken
 * paging; v2 (Server/DC) POSTs to /rest/api/2/search with startAt paging.
 */
import { CliExit, numberValue, writeStdout } from "@toolu/core/cli";
import { formatJson } from "@toolu/core/rest";
import { readFlags, unknownOption } from "./flags.ts";
import { type Conn, TOOL, call, printLean } from "./http.ts";
import { get, rows } from "./jq.ts";
import { paginate } from "./paginate.ts";

const USAGE = "Usage: jira search -q <JQL> [-n MAX] [--all] [--fields f1,f2]";

/** `{issues: [.issues[] | {key, summary, status}]}`, shared by search, board and sprint. */
export function issueRows(value: unknown): unknown {
  return {
    issues: rows(value, "issues", (issue) => ({
      key: get(issue, "key"),
      summary: get(issue, "fields", "summary"),
      status: get(issue, "fields", "status", "name"),
    })),
  };
}

export async function search(conn: Conn, argv: readonly string[]): Promise<number> {
  const flags = readFlags(
    argv,
    {
      values: { "-q": "jql", "--query": "jql", "-n": "max", "--num": "max", "--fields": "fields" },
      switches: { "--all": "all" },
    },
    (arg, parsed) => {
      // A bare argument is the JQL until one is set.
      if ((parsed.values.get("jql") ?? "") !== "") unknownOption("search", arg);
      parsed.values.set("jql", arg);
    },
  );
  const jql = flags.values.get("jql") ?? "";
  if (jql === "") throw new CliExit(1, USAGE);
  const body: Record<string, unknown> = {
    jql,
    maxResults: numberValue(TOOL, "-n", flags.values.get("max") ?? "50"),
  };
  const fields = flags.values.get("fields") ?? "";
  if (fields !== "") body["fields"] = fields.split(",");
  const [path, mode] =
    conn.version === "2"
      ? (["/rest/api/2/search", "offset"] as const)
      : (["/rest/api/3/search/jql", "token"] as const);
  if (flags.switches.has("all")) {
    await writeStdout(formatJson(await paginate(conn, mode, path, body, "issues")));
  } else {
    await printLean(conn, await call(conn, "POST", path, body), issueRows);
  }
  return 0;
}
