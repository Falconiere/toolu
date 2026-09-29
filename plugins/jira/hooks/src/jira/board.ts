/** Agile boards: list, get, a board's issues. Always /rest/agile/1.0, whatever the API version. */
import { CliExit } from "@toolu/core/cli";
import { encodeQuery } from "@toolu/core/rest";
import { type Action, onlyFlags, route, shiftArg } from "./flags.ts";
import { type Conn, call, printLean } from "./http.ts";
import { pick, rows } from "./jq.ts";
import { issueRows } from "./search.ts";

const AGILE = "/rest/agile/1.0";

async function list(conn: Conn, argv: readonly string[]): Promise<number> {
  const flags = onlyFlags("board list", argv, {
    values: { "-p": "project", "--project": "project" },
  });
  const project = flags.values.get("project") ?? "";
  const path = `${AGILE}/board${project === "" ? "" : `?projectKeyOrId=${project}`}`;
  await printLean(conn, await call(conn, "GET", path), (value) => ({
    values: rows(value, "values", (board) => pick(board, "id", "name", "type")),
  }));
  return 0;
}

async function getBoard(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira board get <ID>");
  await printLean(conn, await call(conn, "GET", `${AGILE}/board/${id}`));
  return 0;
}

async function issues(conn: Conn, argv: readonly string[]): Promise<number> {
  const [id, rest] = shiftArg(argv);
  if (id === "") throw new CliExit(1, "Usage: jira board issues <ID> [-q JQL] [-n MAX]");
  const flags = onlyFlags("board issues", rest, {
    values: { "-q": "jql", "--query": "jql", "-n": "max", "--num": "max" },
  });
  const params: Array<[string, string]> = [];
  const jql = flags.values.get("jql") ?? "";
  const max = flags.values.get("max") ?? "";
  if (jql !== "") params.push(["jql", jql]);
  if (max !== "") params.push(["maxResults", max]);
  const text = await call(conn, "GET", `${AGILE}/board/${id}/issue${encodeQuery(params)}`);
  await printLean(conn, text, issueRows);
  return 0;
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = { list, get: getBoard, issues };

export function board(conn: Conn, argv: readonly string[]): Promise<number> {
  return route("Usage: jira board <list|get|issues> ...", ACTIONS, conn, argv);
}
