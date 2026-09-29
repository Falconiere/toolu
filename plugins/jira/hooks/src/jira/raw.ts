/**
 * The escape hatch: any method to any path relative to JIRA_BASE_URL, reaching
 * endpoints the families do not wrap. A body is sent exactly as given.
 */
import { CliExit } from "@toolu/core/cli";
import { type Conn, call, printLean } from "./http.ts";

export async function raw(conn: Conn, argv: readonly string[]): Promise<number> {
  const [method, path, body = ""] = argv;
  if (method === undefined || path === undefined) {
    throw new CliExit(1, "Usage: jira raw <GET|POST|PUT|DELETE> <path> [json_body]");
  }
  await printLean(conn, await call(conn, method, path, body === "" ? undefined : body));
  return 0;
}
