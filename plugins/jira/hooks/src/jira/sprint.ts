/**
 * Agile sprints: list (per board), get, issues, create, move issues, start,
 * complete. Always /rest/agile/1.0; start/complete mutate the live sprint.
 */
import { CliExit, numberValue } from "@toolu/core/cli";
import { type Action, onlyFlags, route, shiftArg } from "./flags.ts";
import { type Conn, TOOL, call, mutate, printLean } from "./http.ts";
import { pick, rows } from "./jq.ts";
import { issueRows } from "./search.ts";

const SPRINT = "/rest/agile/1.0/sprint";

async function list(conn: Conn, [board = ""]: readonly string[]): Promise<number> {
  if (board === "") throw new CliExit(1, "Usage: jira sprint list <BOARD_ID>");
  const text = await call(conn, "GET", `/rest/agile/1.0/board/${board}/sprint`);
  await printLean(conn, text, (value) => ({
    values: rows(value, "values", (sprint) => pick(sprint, "id", "name", "state")),
  }));
  return 0;
}

async function getSprint(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira sprint get <ID>");
  await printLean(conn, await call(conn, "GET", `${SPRINT}/${id}`));
  return 0;
}

async function issues(conn: Conn, [id = ""]: readonly string[]): Promise<number> {
  if (id === "") throw new CliExit(1, "Usage: jira sprint issues <ID>");
  await printLean(conn, await call(conn, "GET", `${SPRINT}/${id}/issue`), issueRows);
  return 0;
}

async function create(conn: Conn, argv: readonly string[]): Promise<number> {
  const [board, rest] = shiftArg(argv);
  const flags = onlyFlags("sprint create", rest, { values: { "-n": "name", "--name": "name" } });
  const name = flags.values.get("name") ?? "";
  if (board === "" || name === "") {
    throw new CliExit(1, "Usage: jira sprint create <BOARD_ID> -n <NAME>");
  }
  // bash passed the id through `jq --argjson`, which exits 2 on a non-number;
  // a board id is also never fractional, so that exits 2 before any request too.
  const originBoardId = numberValue(TOOL, "BOARD_ID", board);
  if (!Number.isInteger(originBoardId))
    throw new CliExit(2, `${TOOL}: BOARD_ID must be an integer`);
  const body = { originBoardId, name };
  await printLean(conn, await call(conn, "POST", SPRINT, body));
  return 0;
}

async function move(conn: Conn, argv: readonly string[]): Promise<number> {
  const [id, keys] = shiftArg(argv);
  if (id === "" || keys.length === 0) {
    throw new CliExit(1, "Usage: jira sprint move <SPRINT_ID> <KEY...>");
  }
  await printLean(conn, await call(conn, "POST", `${SPRINT}/${id}/issue`, { issues: keys }));
  return 0;
}

function setState(state: "active" | "closed", verb: string, done: string): Action<Conn> {
  return async (conn, [id = ""]) => {
    if (id === "") throw new CliExit(1, `Usage: jira sprint ${verb} <ID>`);
    return mutate(conn, "POST", `${SPRINT}/${id}`, { state }, `${done} sprint ${id}`);
  };
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = {
  list,
  get: getSprint,
  issues,
  create,
  move,
  start: setState("active", "start", "started"),
  complete: setState("closed", "complete", "completed"),
};

export function sprint(conn: Conn, argv: readonly string[]): Promise<number> {
  const usage = "Usage: jira sprint <list|get|issues|create|move|start|complete> ...";
  return route(usage, ACTIONS, conn, argv);
}
