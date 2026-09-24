#!/usr/bin/env bun
/** Launch or resume one sub-issue: herdr worktree -> Claude agent -> worker brief. */

import { readFileSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { CommandError, REF_DIR, SCRIPTS_DIR, herdr, readJson, run, writeJson } from "./common.ts";

const START_PROMPT =
  "You are an epic worker. Read {brief} and follow it exactly, starting at Pipeline step 1. " +
  "Report status with the command it gives.";
const RESUME_PROMPT =
  "You are an epic worker resuming interrupted work. Read {brief} and follow it. First inspect " +
  "{status}, `git log origin/{base}..HEAD`, `git status`, and any open PR for this branch, then " +
  "continue from the last reported phase instead of starting over.";

const REPORT_PATH = join(SCRIPTS_DIR, "report.sh");
const BRIEF_TEMPLATE = join(REF_DIR, "worker-brief.md");

type Graph = {
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
  number: number;
  status: string;
  open_blockers: string[];
  blockers: Record<string, string>;
  branch: string;
  checkout: string | null;
};

type LaunchOpts = {
  kind: string;
  model?: string;
  permissionMode: string;
  dryRun: boolean;
  force: boolean;
  reprompt: boolean;
};

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
): string {
  const closed = Object.entries(issue.blockers)
    .filter(([, s]) => s === "closed")
    .map(([b]) => b);
  const values: Record<string, string> = {
    ISSUE_REF: issue.ref,
    ISSUE_URL: issue.url,
    ISSUE_TITLE: issue.title,
    ISSUE_REPO: issue.repo,
    ISSUE_NUMBER: String(issue.number),
    EPIC_REF: graph.epic.ref,
    EPIC_URL: graph.epic.url,
    EPIC_TITLE: graph.epic.title,
    WORKTREE: paths.worktree,
    BRANCH: issue.branch,
    BASE: base,
    REPORT: REPORT_PATH,
    STATUS_FILE: paths.status,
    BLOCKERS: closed.join(", ") || "none",
  };
  let text = readFileSync(BRIEF_TEMPLATE, "utf8");
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

async function ensureAgent(
  key: string,
  pane: string,
  opts: LaunchOpts,
  log: string[],
): Promise<boolean> {
  if (!opts.dryRun) {
    const live = ((await herdr(["agent", "list"])).agents as { name?: string }[] | undefined) ?? [];
    if (live.some((a) => a.name === key)) {
      log.push(`agent ${key} already live`);
      return false;
    }
  }
  const agentArgs = ["--permission-mode", opts.permissionMode, "-n", key];
  if (opts.model) agentArgs.push("--model", opts.model);
  const cmd = [
    "agent",
    "start",
    key,
    "--kind",
    opts.kind,
    "--pane",
    pane,
    "--timeout",
    "90000",
    "--",
    ...agentArgs,
  ];
  log.push("herdr " + shellJoin(cmd));
  if (!opts.dryRun) await herdr(cmd);
  return true;
}

async function prepareCheckout(
  graph: Graph,
  issue: GraphIssue,
  dry: boolean,
  log: string[],
): Promise<[string, string]> {
  let checkout = issue.checkout;
  if (!checkout) {
    const repoName = issue.repo.split("/")[1];
    if (!repoName) throw new Error(`bad repo: ${issue.repo}`);
    checkout = join(graph.clone_root, repoName);
    const cmd = ["gh", "repo", "clone", issue.repo, checkout];
    log.push(shellJoin(cmd));
    if (!dry) await run(cmd);
  }
  const base = (
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
  const fetch = ["git", "-C", checkout, "fetch", "origin", base];
  log.push(shellJoin(fetch));
  if (!dry) await run(fetch);
  return [checkout, base];
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
  const [checkout, base] = await prepareCheckout(graph, issue, dry, log);
  const wt = await ensureWorktree(checkout, issue, base, dry, log);
  const started = await ensureAgent(issue.key, wt.pane_id, opts, log);
  const paths = {
    worktree: wt.worktree,
    status: join(state, "status", `${issue.key}.json`),
    brief: join(state, "briefs", `${issue.key}.md`),
  };
  const brief = renderBrief(graph, issue, paths, base);
  const resuming = Object.keys(record).length > 0 || issue.status === "in_flight";
  const promptTpl = resuming ? RESUME_PROMPT : START_PROMPT;
  const prompt = promptTpl
    .replace("{brief}", paths.brief)
    .replace("{status}", paths.status)
    .replace("{base}", base);
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
    stage: "running",
    status_file: paths.status,
    brief: paths.brief,
    launched_at: record.launched_at ?? now,
    last_launch: now,
    launches: (typeof record.launches === "number" ? record.launches : 0) + 1,
  });
  await writeJson(join(state, "issues", `${issue.key}.json`), record);
  return { issue: issue.ref, commands: log, ...record };
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let graphPath: string | undefined;
  let issueRef: string | undefined;
  const opts: LaunchOpts = {
    kind: "claude",
    permissionMode: "auto",
    dryRun: false,
    force: false,
    reprompt: false,
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
    } else if (a === "--permission-mode") {
      const v = argv[++i];
      if (v) opts.permissionMode = v;
    } else if (a === "--dry-run") opts.dryRun = true;
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
