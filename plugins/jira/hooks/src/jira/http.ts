/**
 * Jira transport over @toolu/core/rest: credential and version resolution
 * (bash `jira_require_env`), the authenticated request (`jira_curl`), and the
 * two output shapes (`jira_lean`, or the raw body `jira_curl` printed alone).
 */
import { CliExit, writeStdout } from "@toolu/core/cli";
import { jsonOutput, parseJson, send, type RestRequest } from "@toolu/core/rest";
import { type Env, withCliCreds } from "./creds.ts";
import { CREDS_HELP } from "./usage.ts";

export const TOOL = "jira";

export interface Conn {
  /** JIRA_BASE_URL without trailing slashes. */
  readonly base: string;
  readonly authorization: string;
  readonly version: "2" | "3";
  readonly lean: boolean;
  /** The environment plan checks inherit: the caller's, with jira-cli creds filled in. */
  readonly env: Env;
}

/** Global flags as the dispatcher resolved them; empty version means "not given". */
export interface Globals {
  readonly version: string;
  readonly lean: boolean;
}

function authorization(env: Env): string | undefined {
  if (env["JIRA_PAT"]) return `Bearer ${env["JIRA_PAT"]}`;
  const email = env["JIRA_EMAIL"];
  const token = env["JIRA_API_TOKEN"];
  if (!email || !token) return undefined;
  return `Basic ${Buffer.from(`${email}:${token}`).toString("base64")}`;
}

/**
 * Resolves base, auth and API version, or exits 1: the friendly setup text
 * when no credentials resolve, a named error for a bad version. `_JIRA_VER` /
 * `_JIRA_LEAN` are inherited from a parent jira (a plan check), as in bash.
 */
export function connect(processEnv: Env, globals: Globals): Conn {
  const env = withCliCreds(processEnv);
  const base = (env["JIRA_BASE_URL"] ?? "").replace(/\/+$/, "");
  const auth = authorization(env);
  if (!env["JIRA_BASE_URL"] || auth === undefined) throw new CliExit(1, CREDS_HELP);
  const version = globals.version || env["_JIRA_VER"] || env["JIRA_API_VERSION"] || "3";
  if (version !== "2" && version !== "3") {
    throw new CliExit(1, `jira: api version must be 2 or 3 (got '${version}')`);
  }
  const lean = globals.lean || env["_JIRA_LEAN"] === "1";
  return { base, authorization: auth, version, lean, env };
}

/** `/rest/api/<version>`. */
export function api(conn: Conn): string {
  return `/rest/api/${conn.version}`;
}

/** The request `jira_curl` built: auth, JSON Accept, and a JSON Content-Type with a body. */
export function jiraRequest(conn: Conn, method: string, path: string, body?: unknown): RestRequest {
  const headers: Record<string, string> = {
    Accept: "application/json",
    Authorization: conn.authorization,
  };
  const request = { url: `${conn.base}${path}`, method, headers };
  if (body === undefined) return request;
  headers["Content-Type"] = "application/json";
  return typeof body === "string" ? { ...request, payload: body } : { ...request, body };
}

/** One request; a string body is sent verbatim, anything else JSON-encoded. */
export function call(conn: Conn, method: string, path: string, body?: unknown): Promise<string> {
  return send(TOOL, jiraRequest(conn, method, path, body));
}

/**
 * A request whose body only feeds a later step (bash captured it with `$(…)`):
 * an HTTP error exits `code` with no stdout, and the body is returned parsed.
 */
export async function lookup(
  conn: Conn,
  path: string,
  options: { method?: string; body?: unknown; code?: number } = {},
): Promise<unknown> {
  let text: string;
  try {
    text = await call(conn, options.method ?? "GET", path, options.body);
  } catch (error) {
    if (error instanceof CliExit && error.code === 22) {
      throw new CliExit(options.code ?? 1, error.message);
    }
    throw error;
  }
  return parseJson(TOOL, text);
}

/** `| jira_lean '<projection>'`: the projection under --lean, else the whole body. */
export async function printLean(
  conn: Conn,
  text: string,
  projection: (value: unknown) => unknown = (value) => value,
): Promise<void> {
  await writeStdout(conn.lean ? jsonOutput(TOOL, text, projection) : jsonOutput(TOOL, text));
}

/** A mutation bash ran as `jira_curl …; echo "<done>"`: the raw body, then the line. */
export async function mutate(
  conn: Conn,
  method: string,
  path: string,
  body: unknown,
  done: string,
): Promise<number> {
  const text = await send(TOOL, { ...jiraRequest(conn, method, path, body), json: false });
  await writeStdout(`${text}${done}\n`);
  return 0;
}
