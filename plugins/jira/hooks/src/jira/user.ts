/** Users: whoami, search, get. v3 keys users by accountId, v2 by username. */
import { CliExit } from "@toolu/core/cli";
import { encodeQuery } from "@toolu/core/rest";
import { type Action, onlyFlags, route } from "./flags.ts";
import { type Conn, api, call, printLean } from "./http.ts";
import { iterate, pick } from "./jq.ts";

async function whoami(conn: Conn): Promise<number> {
  await printLean(conn, await call(conn, "GET", `${api(conn)}/myself`), (value) =>
    pick(value, "accountId", "displayName", "emailAddress"),
  );
  return 0;
}

async function search(conn: Conn, argv: readonly string[]): Promise<number> {
  const flags = onlyFlags("user search", argv, { values: { "-q": "query", "--query": "query" } });
  const query = flags.values.get("query") ?? "";
  if (query === "") throw new CliExit(1, "Usage: jira user search -q <QUERY>");
  const param = conn.version === "2" ? "username" : "query";
  const text = await call(conn, "GET", `${api(conn)}/user/search${encodeQuery([[param, query]])}`);
  await printLean(conn, text, (value) => ({
    users: iterate(value).map((user) => pick(user, "accountId", "displayName")),
  }));
  return 0;
}

async function getUser(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira user get <ACCOUNT_ID>");
  const param = conn.version === "2" ? "username" : "accountId";
  await printLean(conn, await call(conn, "GET", `${api(conn)}/user${encodeQuery([[param, id]])}`));
  return 0;
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = { whoami, search, get: getUser };

export function user(conn: Conn, argv: readonly string[]): Promise<number> {
  return route("Usage: jira user <whoami|search|get> ...", ACTIONS, conn, argv);
}
