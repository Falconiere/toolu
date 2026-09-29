/** Worklogs: add (optional comment, ADF for v3), list, delete. */
import { CliExit } from "@toolu/core/cli";
import { textBody } from "./adf.ts";
import { type Action, onlyFlags, route, shiftArg } from "./flags.ts";
import { type Conn, api, call, mutate, printLean } from "./http.ts";
import { get, pick, rows } from "./jq.ts";

async function add(conn: Conn, argv: readonly string[]): Promise<number> {
  const [key, rest] = shiftArg(argv);
  const flags = onlyFlags("worklog add", rest, {
    values: { "-t": "time", "--time": "time", "-c": "comment", "--comment": "comment" },
  });
  const time = flags.values.get("time") ?? "";
  if (key === "" || time === "") {
    throw new CliExit(1, "Usage: jira worklog add <KEY> -t <TIME> [-c COMMENT]");
  }
  const body: Record<string, unknown> = { timeSpent: time };
  // An explicit -c "" still sends a comment, as bash's has_comment did.
  const comment = flags.values.get("comment");
  if (comment !== undefined) body["comment"] = textBody(conn.version, comment);
  await printLean(conn, await call(conn, "POST", `${api(conn)}/issue/${key}/worklog`, body));
  return 0;
}

async function list(conn: Conn, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "Usage: jira worklog list <KEY>");
  const text = await call(conn, "GET", `${api(conn)}/issue/${key}/worklog`);
  await printLean(conn, text, (value) => ({
    worklogs: rows(value, "worklogs", (entry) => ({
      id: get(entry, "id"),
      author: get(entry, "author", "displayName"),
      ...pick(entry, "timeSpent", "started"),
    })),
  }));
  return 0;
}

async function remove(conn: Conn, [key = "", id = ""]: readonly string[]): Promise<number> {
  if (key === "" || id === "") {
    throw new CliExit(1, "Usage: jira worklog delete <KEY> <WORKLOG_ID>");
  }
  const path = `${api(conn)}/issue/${key}/worklog/${id}`;
  return mutate(conn, "DELETE", path, undefined, `deleted worklog ${id}`);
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = { add, list, delete: remove };

export function worklog(conn: Conn, argv: readonly string[]): Promise<number> {
  return route("Usage: jira worklog <add|list|delete> ...", ACTIONS, conn, argv);
}
