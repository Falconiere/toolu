/** Projects: list, get, versions, components. All read-only GETs. */
import { CliExit } from "@toolu/core/cli";
import { type Action, route } from "./flags.ts";
import { type Conn, api, call, printLean } from "./http.ts";
import { iterate, pick } from "./jq.ts";

async function list(conn: Conn): Promise<number> {
  await printLean(conn, await call(conn, "GET", `${api(conn)}/project`), (value) => ({
    projects: iterate(value).map((project) => pick(project, "key", "name", "id")),
  }));
  return 0;
}

/** `<action> <KEY>`: GET the project, or one of its sub-collections, in full. */
function detail(action: string, suffix: string): Action<Conn> {
  return async (conn, [key = ""]) => {
    if (key === "") throw new CliExit(1, `Usage: jira project ${action} <KEY>`);
    await printLean(conn, await call(conn, "GET", `${api(conn)}/project/${key}${suffix}`));
    return 0;
  };
}

const ACTIONS: Readonly<Record<string, Action<Conn>>> = {
  list,
  get: detail("get", ""),
  versions: detail("versions", "/versions"),
  components: detail("components", "/components"),
};

export function project(conn: Conn, argv: readonly string[]): Promise<number> {
  return route("Usage: jira project <list|get|versions|components> ...", ACTIONS, conn, argv);
}
