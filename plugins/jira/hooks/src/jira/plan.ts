/**
 * The `plan` family: scaffold a plan doc for a ticket, run its checks, and
 * keep a dashboard-readable ledger at <repo>/<host-dir>/tmp/plan-ledger/jira-<KEY>.json.
 */
import { CliExit, writeStdout } from "@toolu/core/cli";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import type { Env } from "./creds.ts";
import { type Action, route } from "./flags.ts";
import { type Conn, api, lookup } from "./http.ts";
import { alt, get, iterate, text } from "./jq.ts";
import { issueKey } from "./plan-parse.ts";
import { runPlan } from "./plan-run.ts";
import { docPath, ledgerPath, readLedger } from "./plan-store.ts";
import { PLAN_USAGE } from "./usage.ts";

export interface PlanContext {
  /** Undefined only for `plan path`, which never reaches Jira. */
  readonly conn: Conn | undefined;
  /** What plan checks inherit. */
  readonly env: Env;
  readonly cwd: string;
}

function connected(context: PlanContext): Conn {
  if (context.conn === undefined) throw new CliExit(1, "jira plan: no Jira connection");
  return context.conn;
}

function template(key: string, summary: string, doc: string): string {
  const date = new Date().toISOString().slice(0, 10);
  return `# ${key} — ${summary}

**Date:** ${date}   **Issue:** ${key}   **Topic:** ${summary}

## Steps (machine-readable)

\`\`\`json
[]
\`\`\`

Fill the array with {"id","title","check"} objects. A \`check\` is a shell command
that exits 0 **only when Jira itself reflects the change** — assert against a live
read, e.g.

    "$JIRA" issue get ${key} --lean | jq -e '.status=="Done"' >/dev/null

\`$JIRA\` is bound to the jira CLI when the check runs. Then: jira.sh plan run ${doc}
`;
}

/** Writes <repo>/<host-dir>/tmp/jira/plans/<KEY>.md titled from a live issue read; never clobbers. */
async function init(context: PlanContext, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "jira plan init: needs an issue key");
  const doc = docPath(key, context.env, context.cwd);
  if (existsSync(doc)) throw new CliExit(1, `jira plan init: ${doc} already exists`);
  const conn = connected(context);
  const issue = await lookup(conn, `${api(conn)}/issue/${key}`);
  const summary = text(alt(get(issue, "fields", "summary"), "")) || key;
  mkdirSync(dirname(doc), { recursive: true });
  try {
    // Exclusive create: a doc (or symlink) that appeared since the check is never written through.
    writeFileSync(doc, template(key, summary, doc), { flag: "wx" });
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "EEXIST") {
      throw new CliExit(1, `jira plan init: ${doc} already exists`);
    }
    throw error;
  }
  await writeStdout(`${doc}\n`);
  return 0;
}

async function status(context: PlanContext, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "jira plan status: needs an issue key");
  const ledger = readLedger(ledgerPath(key, context.env, context.cwd));
  if (ledger === undefined) {
    throw new CliExit(1, `jira plan status: no ledger for ${key} (run: jira.sh plan run <doc>)`);
  }
  const summary = get(ledger, "summary");
  const head =
    `${text(get(ledger, "branch"))}  ${text(get(summary, "green"))}/${text(get(summary, "total"))}` +
    ` green   next: ${text(alt(get(ledger, "next"), "-"))}`;
  const steps = iterate(get(ledger, "steps")).map(
    (step) =>
      `  ${text(get(step, "status"))}\t${text(get(step, "id"))}\t${text(get(step, "title"))}`,
  );
  await writeStdout(`${[head, ...steps].join("\n")}\n`);
  return 0;
}

/** Prints the ledger path; needs no credentials. */
async function path(context: PlanContext, [key = ""]: readonly string[]): Promise<number> {
  if (key === "") throw new CliExit(1, "jira plan path: needs an issue key");
  await writeStdout(`${ledgerPath(key, context.env, context.cwd)}\n`);
  return 0;
}

async function run(context: PlanContext, argv: readonly string[]): Promise<number> {
  const [doc = "", ...rest] = argv;
  if (doc === "") throw new CliExit(1, "jira plan run: needs a plan doc path");
  const key = issueKey(doc);
  return runPlan({ key, doc, env: context.env, cwd: context.cwd }, rest);
}

const ACTIONS: Readonly<Record<string, Action<PlanContext>>> = { init, status, path, run };

export function plan(context: PlanContext, argv: readonly string[]): Promise<number> {
  return route(PLAN_USAGE, ACTIONS, context, argv);
}
