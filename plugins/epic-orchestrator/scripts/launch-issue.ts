/** Launch or resume one sub-issue: herdr worktree -> routed host agent -> worker brief. */

import { readFileSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { CommandError, REF_DIR, SCRIPTS_DIR, herdr, readJson, run, writeJson } from "./common.ts";
import { agentArgs, hostKind, skillRef, type HostKind } from "./hosts.ts";
import { budgetLow, ghBudget } from "./ratelimit.ts";
import type { Route } from "./route.ts";

const START_PROMPT =
  "You are an epic worker. Read {brief} and follow it exactly, starting at Pipeline step 1. " +
  "Report status with the command it gives.";
const RESUME_PROMPT =
  "You are an epic worker resuming interrupted work. Read {brief} and follow it. First inspect " +
  "{status}, `git log origin/{base}..HEAD`, `git status`, and any open PR for this branch, then " +
  "continue from the last reported phase instead of starting over.";

// Run through bun, like every other script: no host cache has to keep an exec bit.
const REPORT_COMMAND = `bun "${join(SCRIPTS_DIR, "report.ts")}"`;
const BRIEF_TEMPLATE = join(REF_DIR, "worker-brief.md");

type Graph = {
  tracker?: string;
  epic: { ref: string; url: string; title: string };
  state_dir: string;
  clone_root: string;
  issues: GraphIssue[];
};

type GraphIssue = {
  ref: string;
  key: string;
  url: string;
  title: string;
  repo: string;
  number: number | null;
  status: string;
  open_blockers: string[];
  blockers: Record<string, string>;
  branch: string;
  checkout: string | null;
};

type LaunchOpts = {
  /** Explicit host; otherwise the issue's route, otherwise claude. */
  kind?: string;
  model?: string;
  effort?: string;
  permissionMode: string;
  /** Keep approval prompts on (attended run). Default is unattended. */
  safe: boolean;
  dryRun: boolean;
  force: boolean;
  reprompt: boolean;
  /** Exit a live agent first, e.g. to move the issue to another host. */
  replace: boolean;
};

type Resolved = { kind: HostKind; model?: string | undefined; effort?: string | undefined };

/** Explicit flags win; a route supplies host, model, and effort; a model or
 * effort from the route applies only when its host is the one launching. */
export function resolveHost(opts: LaunchOpts, route: Route | null): Resolved {
  const kind = hostKind(opts.kind ?? route?.host ?? "claude");
  const fromRoute = route?.host === kind ? route : null;
  return {
    kind,
    model: opts.model ?? fromRoute?.model ?? undefined,
    effort: opts.effort ?? fromRoute?.effort ?? undefined,
  };
}

/** How the worker reads its issue and what its PR body must say, per tracker. */
function trackerLines(graph: Graph, issue: GraphIssue): { read: string; closes: string } {
  if (graph.tracker === "jira") {
    return {
      read: `\`jira.sh issue get ${issue.ref}\` (the toolu jira skill)`,
      closes: `Resolves ${issue.ref}`,
    };
  }
  if (graph.tracker === "linear") {
    return { read: `the Linear issue ${issue.url}`, closes: `Fixes ${issue.ref}` };
  }
  // The issue URL and canonical ref (`owner/repo#N`) already carry the number.
  return {
    read: `\`gh issue view ${issue.url} --comments\``,
    closes: `Closes ${issue.ref}`,
  };
}

function shellJoin(argv: string[]): string {
  return argv
    .map((a) => {
      if (a === "") return "''";
      if (/^[A-Za-z0-9_./:=,@%+-]+$/.test(a)) return a;
      return `'${a.replace(/'/g, `'\\''`)}'`;
    })
    .join(" ");
}

export function findIssue(graph: Graph, wanted: string): GraphIssue {
  for (const issue of graph.issues) {
    if (wanted === issue.ref || wanted === issue.key || wanted === issue.url) {
      return issue;
    }
  }
  throw new Error(`issue ${JSON.stringify(wanted)} is not a sub-issue of ${graph.epic.ref}`);
}

export function renderBrief(
  graph: Graph,
  issue: GraphIssue,
  paths: { worktree: string; status: string; brief?: string },
  base: string,
  kind: HostKind = "claude",
): string {
  const lines = trackerLines(graph, issue);
  const closed = Object.entries(issue.blockers)
    .filter(([, s]) => s === "closed")
    .map(([b]) => b);
  const values: Record<string, string> = {
    ISSUE_REF: issue.ref,
    ISSUE_URL: issue.url,
    ISSUE_TITLE: issue.title,
    ISSUE_READ: lines.read,
    CLOSES: lines.closes,
    HOST: kind,
    DELIVERY: skillRef(kind, "delivery-flow", "delivery-flow"),
    BABYSIT: skillRef(kind, "pr-babysit", "babysit"),
    DEBUG: skillRef(kind, "toolu", "debug"),
    EPIC_REF: graph.epic.ref,
    EPIC_URL: graph.epic.url,
    EPIC_TITLE: graph.epic.title,
    WORKTREE: paths.worktree,
    BRANCH: issue.branch,
    BASE: base,
    REPORT: REPORT_COMMAND,
    STATUS_FILE: paths.status,
    BLOCKERS: closed.join(", ") || "none",
  };
  // The leading comment documents the placeholders for maintainers only.
  let text = readFileSync(BRIEF_TEMPLATE, "utf8").replace(/^<!--[\s\S]*?-->\n+/, "");
  for (const [key, val] of Object.entries(values)) {
    text = text.replaceAll(`{{${key}}}`, val);
  }
  return text;
}

async function ensureWorktree(
  checkout: string,
  issue: GraphIssue,
  base: string,
  dry: boolean,
  log: string[],
): Promise<{ workspace_id: string; pane_id: string; worktree: string }> {
  type Wt = {
    branch?: string;
    open_workspace_id?: string;
    path?: string;
  };
  const listed = dry
    ? []
    : (((await herdr(["worktree", "list", "--cwd", checkout])).worktrees as Wt[] | undefined) ??
      []);
  const existing = listed.find((w) => w.branch === issue.branch);
  if (existing?.open_workspace_id) {
    const ws = existing.open_workspace_id;
    const panes = (await herdr(["pane", "list", "--workspace", ws])).panes as {
      pane_id: string;
    }[];
    const pane0 = panes[0];
    if (!pane0) throw new CommandError(`no panes in workspace ${ws}`);
    log.push(`reuse open worktree workspace ${ws}`);
    return {
      workspace_id: ws,
      pane_id: pane0.pane_id,
      worktree: existing.path ?? "",
    };
  }
  let cmd: string[];
  if (existing) {
    cmd = [
      "worktree",
      "open",
      "--cwd",
      checkout,
      "--path",
      existing.path ?? "",
      "--label",
      issue.key,
      "--no-focus",
    ];
  } else {
    cmd = [
      "worktree",
      "create",
      "--cwd",
      checkout,
      "--branch",
      issue.branch,
      "--base",
      `origin/${base}`,
      "--label",
      issue.key,
      "--no-focus",
    ];
  }
  log.push("herdr " + shellJoin(cmd));
  if (dry) {
    return { workspace_id: "<new>", pane_id: "<root-pane>", worktree: "<herdr worktree path>" };
  }
  const res = await herdr(cmd);
  const workspace = res.workspace as { workspace_id: string };
  const rootPane = res.root_pane as { pane_id: string };
  const worktree = res.worktree as { path: string };
  return {
    workspace_id: workspace.workspace_id,
    pane_id: rootPane.pane_id,
    worktree: worktree.path,
  };
}

async function liveAgent(key: string): Promise<boolean> {
  const live = ((await herdr(["agent", "list"])).agents as { name?: string }[] | undefined) ?? [];
  return live.some((a) => a.name === key);
}

/** Ask a live agent to exit and wait until herdr no longer lists it. Local
 * work stays in the worktree; the watcher checkpoints it as well. */
async function stopAgent(key: string, log: string[]): Promise<void> {
  log.push(`herdr agent prompt ${key} /exit`);
  await herdr(["agent", "send-keys", key, "esc"]).catch(() => ({}));
  await herdr(["agent", "prompt", key, "/exit"]).catch(() => ({}));
  for (let i = 0; i < 30; i++) {
    if (!(await liveAgent(key))) return;
    await Bun.sleep(1000);
  }
  throw new CommandError(`agent ${key} did not exit; stop it by hand before --replace`);
}

type AgentPlan = {
  host: Resolved;
  bypass: boolean;
  permissionMode: string;
  /** Previous host for this issue, if it ran before. */
  previousKind: string | undefined;
  launches: number;
};

/** Start (or keep) the worker agent. Returns [started, resumed]. */
async function ensureAgent(
  key: string,
  pane: string,
  plan: AgentPlan,
  opts: LaunchOpts,
  log: string[],
): Promise<[boolean, boolean]> {
  const { kind } = plan.host;
  if (!opts.dryRun && (await liveAgent(key))) {
    const moving = plan.previousKind !== undefined && plan.previousKind !== kind;
    if (!opts.replace && !moving) {
      log.push(`agent ${key} already live`);
      return [false, false];
    }
    await stopAgent(key, log);
  }
  // Same host again: continue its last conversation so no context is lost.
  const resume = plan.launches > 0 && plan.previousKind === kind;
  const start = async (withResume: boolean): Promise<void> => {
    const args = agentArgs(kind, {
      key,
      model: plan.host.model,
      effort: plan.host.effort,
      bypass: plan.bypass,
      permissionMode: plan.permissionMode,
      resume: withResume,
    });
    const cmd = [
      "agent",
      "start",
      key,
      "--kind",
      kind,
      "--pane",
      pane,
      "--timeout",
      "90000",
      "--",
      ...args,
    ];
    log.push("herdr " + shellJoin(cmd));
    if (!opts.dryRun) await herdr(cmd);
  };
  if (!resume) {
    await start(false);
    return [true, false];
  }
  try {
    await start(true);
    return [true, true];
  } catch (err) {
    // No session to continue (history pruned, new machine): start fresh; the
    // resume prompt still rebuilds context from git and the status file.
    if (!(err instanceof CommandError)) throw err;
    log.push(`# resume failed (${err.message.slice(0, 120)}); starting a fresh session`);
    await start(false);
    return [true, false];
  }
}

async function prepareCheckout(
  graph: Graph,
  issue: GraphIssue,
  dry: boolean,
  log: string[],
): Promise<[string, string]> {
  let checkout = issue.checkout;
  if (!checkout) {
    const parts = issue.repo.split("/");
    const repoName = parts[1];
    if (!repoName || parts.length !== 2) throw new Error(`bad repo: ${issue.repo}`);
    checkout = join(graph.clone_root, repoName);
    const cmd = ["gh", "repo", "clone", issue.repo, checkout];
    log.push(shellJoin(cmd));
    if (!dry) await run(cmd);
  }
  // Dry-run must not call the network: CI and offline dry-runs have no access
  // to every epic repo. Live launches still resolve the real default branch.
  let base = "main";
  if (!dry) {
    base = (
      await run([
        "gh",
        "repo",
        "view",
        issue.repo,
        "--json",
        "defaultBranchRef",
        "-q",
        ".defaultBranchRef.name",
      ])
    ).trim();
  } else {
    log.push(`# dry-run: skip gh repo view; assume default branch ${base}`);
  }
  const fetch = ["git", "-C", checkout, "fetch", "origin", base];
  log.push(shellJoin(fetch));
  if (!dry) await run(fetch);
  return [checkout, base];
}

/** New workers each add babysit polling on the same GitHub token; refuse to
 * start one when the budget is already under its floor. */
async function guardBudget(): Promise<void> {
  const budget = await ghBudget();
  const low = budget ? budgetLow(budget) : null;
  if (low && budget) {
    const reset = new Date(Math.min(budget.core.reset, budget.graphql.reset)).toISOString();
    throw new CommandError(`${low}; resets by ${reset}. Launch later, or pass --force.`);
  }
}

async function launch(
  graph: Graph,
  issue: GraphIssue,
  opts: LaunchOpts,
): Promise<Record<string, unknown>> {
  const dry = opts.dryRun;
  const log: string[] = [];
  if (!["ready", "in_flight"].includes(issue.status) && !opts.force) {
    throw new Error(
      `${issue.ref} is ${issue.status} (open blockers: ${JSON.stringify(issue.open_blockers)}); ` +
        "pass --force to launch anyway",
    );
  }
  const state = graph.state_dir;
  const record = readJson<Record<string, unknown>>(join(state, "issues", `${issue.key}.json`), {});
  const route = readJson<Route | null>(join(state, "routes", `${issue.key}.json`), null);
  const host = resolveHost(opts, route);
  const launches = typeof record.launches === "number" ? record.launches : 0;
  if (!dry && launches === 0 && !opts.force) await guardBudget();
  const [checkout, base] = await prepareCheckout(graph, issue, dry, log);
  const wt = await ensureWorktree(checkout, issue, base, dry, log);
  const plan: AgentPlan = {
    host,
    bypass: !opts.safe,
    permissionMode: opts.permissionMode,
    previousKind: typeof record.kind === "string" ? record.kind : undefined,
    launches,
  };
  const [started, resumed] = await ensureAgent(issue.key, wt.pane_id, plan, opts, log);
  const paths = {
    worktree: wt.worktree,
    status: join(state, "status", `${issue.key}.json`),
    brief: join(state, "briefs", `${issue.key}.md`),
  };
  const brief = renderBrief(graph, issue, paths, base, host.kind);
  const resuming = Object.keys(record).length > 0 || issue.status === "in_flight";
  const promptTpl = resuming ? RESUME_PROMPT : START_PROMPT;
  const prompt = promptTpl
    .replace("{brief}", paths.brief)
    .replace("{status}", paths.status)
    .replace("{base}", base)
    .concat(resumed ? " Your previous conversation for this issue is loaded above." : "");
  const promptCmd = [
    "agent",
    "prompt",
    issue.key,
    prompt,
    "--wait",
    "--until",
    "working",
    "--until",
    "blocked",
    "--timeout",
    "60000",
  ];
  if (started || opts.reprompt) log.push("herdr " + shellJoin(promptCmd));
  if (dry) {
    return {
      dry_run: true,
      issue: issue.ref,
      host,
      commands: log,
      brief_path: paths.brief,
      brief,
    };
  }
  await mkdir(dirname(paths.brief), { recursive: true });
  await writeFile(paths.brief, brief);
  if (started || opts.reprompt) await herdr(promptCmd);
  const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
  for (const k of ["ref", "key", "repo", "number", "url", "title", "branch"] as const) {
    record[k] = issue[k];
  }
  Object.assign(record, {
    base,
    checkout,
    ...wt,
    agent: issue.key,
    kind: host.kind,
    model: host.model ?? null,
    effort: host.effort ?? null,
    bypass: !opts.safe,
    stage: "running",
    status_file: paths.status,
    brief: paths.brief,
    launched_at: record.launched_at ?? now,
    last_launch: now,
    launches: launches + 1,
  });
  await writeJson(join(state, "issues", `${issue.key}.json`), record);
  return { issue: issue.ref, commands: log, ...record };
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let graphPath: string | undefined;
  let issueRef: string | undefined;
  const opts: LaunchOpts = {
    permissionMode: "auto",
    safe: false,
    dryRun: false,
    force: false,
    reprompt: false,
    replace: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--graph") graphPath = argv[++i];
    else if (a === "--issue") issueRef = argv[++i];
    else if (a === "--kind") {
      const v = argv[++i];
      if (v) opts.kind = v;
    } else if (a === "--model") {
      const v = argv[++i];
      if (v !== undefined) opts.model = v;
    } else if (a === "--effort") {
      const v = argv[++i];
      if (v !== undefined) opts.effort = v;
    } else if (a === "--permission-mode") {
      const v = argv[++i];
      if (v) opts.permissionMode = v;
      opts.safe = true;
    } else if (a === "--safe") opts.safe = true;
    else if (a === "--replace") opts.replace = true;
    else if (a === "--dry-run") opts.dryRun = true;
    else if (a === "--force") opts.force = true;
    else if (a === "--reprompt") opts.reprompt = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!graphPath || !issueRef) {
    throw new Error("usage: launch-issue.ts --graph GRAPH.json --issue REF [--dry-run] ...");
  }
  const graph = JSON.parse(await readFile(graphPath, "utf8")) as Graph;
  let result: Record<string, unknown>;
  try {
    result = await launch(graph, findIssue(graph, issueRef), opts);
  } catch (err) {
    if (err instanceof CommandError) {
      process.stderr.write(JSON.stringify({ issue: issueRef, error: String(err) }) + "\n");
      process.exit(1);
    }
    if (err instanceof Error) {
      process.stderr.write(err.message + "\n");
      process.exit(1);
    }
    throw err;
  }
  if (opts.dryRun) {
    const commands = result.commands as string[];
    process.stdout.write(
      [
        "# " + String(result.issue),
        ...commands,
        `# brief -> ${String(result.brief_path)}`,
        "",
        String(result.brief),
      ].join("\n") + "\n",
    );
  } else {
    process.stdout.write(JSON.stringify(result, null, 2) + "\n");
  }
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
