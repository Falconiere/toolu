#!/usr/bin/env bun
/**
 * Jira CLI — the TypeScript port of the bash jira.sh (#272): same families,
 * flags, output and exit statuses, against Jira Cloud and Server/Data Center.
 * Global flags (--api-version, --lean) are taken from anywhere in argv, then
 * <family> <action> routes to its module. Credentials come from JIRA_* env or
 * an installed jira-cli, never from .env.
 */
import { CliExit, flagValue, runCli } from "@toolu/core/cli";
import { attachment } from "./jira/attachment.ts";
import { board } from "./jira/board.ts";
import { type Conn, connect, type Globals } from "./jira/http.ts";
import { issue } from "./jira/issue.ts";
import { plan } from "./jira/plan.ts";
import { project } from "./jira/project.ts";
import { raw } from "./jira/raw.ts";
import { search } from "./jira/search.ts";
import { sprint } from "./jira/sprint.ts";
import { user } from "./jira/user.ts";
import { USAGE } from "./jira/usage.ts";
import { worklog } from "./jira/worklog.ts";

type Family = (conn: Conn, argv: readonly string[]) => Promise<number>;

const FAMILIES: Readonly<Record<string, Family>> = {
  search,
  issue,
  board,
  sprint,
  worklog,
  project,
  user,
  attachment,
  raw,
};

/** Strips the global flags from anywhere in argv, as the bash dispatcher did. */
function splitGlobals(argv: readonly string[]): { globals: Globals; args: string[] } {
  const args: string[] = [];
  let version = "";
  let lean = false;
  for (let at = 0; at < argv.length; at += 1) {
    const arg = argv[at] ?? "";
    if (arg === "--api-version") {
      version = flagValue("jira", argv, at);
      at += 1;
    } else if (arg.startsWith("--api-version=")) {
      version = arg.slice("--api-version=".length);
    } else if (arg === "--lean") {
      lean = true;
    } else {
      args.push(arg);
    }
  }
  return { globals: { version, lean }, args };
}

/** The environment a plan check inherits: the resolved version and lean flag, as bash exported them. */
function childEnv(conn: Conn): Record<string, string | undefined> {
  return { ...conn.env, _JIRA_VER: conn.version, ...(conn.lean ? { _JIRA_LEAN: "1" } : {}) };
}

async function main(): Promise<number> {
  const { globals, args } = splitGlobals(process.argv.slice(2));
  const [family = "", ...rest] = args;
  if (family === "") throw new CliExit(1, USAGE);
  const run = Object.hasOwn(FAMILIES, family) ? FAMILIES[family] : undefined;
  if (run === undefined && family !== "plan") {
    throw new CliExit(1, `jira: unknown family '${family}'\n${USAGE}`);
  }
  // `plan path` only prints a local path, so it needs no credentials; everything else reaches Jira.
  const conn = family === "plan" && rest[0] === "path" ? undefined : connect(process.env, globals);
  if (run !== undefined && conn !== undefined) return run(conn, rest);
  const env = conn === undefined ? process.env : childEnv(conn);
  return plan({ conn, env, cwd: process.cwd() }, rest);
}

await runCli(main);
