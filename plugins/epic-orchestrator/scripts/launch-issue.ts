/** Launch or resume one sub-issue: herdr worktree -> routed host agent -> worker brief. */

import { readFileSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { bindWorktree } from "../hooks/dist/epic-runtime.js";
import { launchReservation, persistAttempt, withLaunchOwnership } from "./admission.ts";
import { acknowledgedOutcome } from "./lifecycle.ts";
import { awaitSession, captureSession } from "./session.ts";
import { ensureAgent, type AgentPlan } from "./launch/agent.ts";
import { loadLaunchRecord } from "./launch-state.ts";
import { loadLaunchRoute } from "./launch-route.ts";
import { ensureWorktree, prepareCheckout, shellJoin } from "./launch-worktree.ts";
import {
  CommandError,
  REF_DIR,
  SCRIPTS_DIR,
  commandExitCode,
  failureDetails,
  herdr,
} from "./common.ts";
import { hostKind, skillRef, type HostKind } from "./hosts.ts";
import { checkOpencode, excludeForWorker } from "./opencode-worker.ts";
import { budgetLow, ghBudget } from "./ratelimit.ts";

export const START_PROMPT =
  "You are an epic worker. Read {brief} and follow it exactly, starting at Pipeline step 1. " +
  "Report status with the command it gives.";
const RESUME_PROMPT =
  "You are an epic worker resuming interrupted work. Read {brief} and follow it. First inspect " +
  "{status}, `git log origin/{base}..HEAD`, `git status`, and any open PR for this branch, then " +
  "continue from the last reported phase instead of starting over.";

// Run through bun, like every other script: no host cache has to keep an exec bit.
const REPORT_COMMAND = `bun "${join(SCRIPTS_DIR, "report.ts")}"`;
const BRIEF_TEMPLATE = join(REF_DIR, "worker-brief.md");

export type Graph = {
  tracker?: string;
  epic: { ref: string; url: string; title: string };
  state_dir: string;
  clone_root: string;
  issues: GraphIssue[];
  max_parallel?: number;
};

export type GraphIssue = {
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
export function resolveHost(opts: LaunchOpts, route: ReturnType<typeof loadLaunchRoute>): Resolved {
  if (route && route.host === null)
    throw new Error("no host capacity: reroute after a slot becomes available");
  const kind = hostKind(opts.kind ?? route?.host ?? "claude");
  const fromRoute = route?.host === kind ? route : null;
  return {
    kind,
    model: opts.model ?? fromRoute?.model ?? undefined,
    effort: opts.effort ?? fromRoute?.effort ?? undefined,
  };
}

/** How the worker reads its issue and what its PR body must say, per tracker. */
function trackerLines(
  graph: Graph,
  issue: GraphIssue,
  kind: HostKind,
): { read: string; closes: string } {
  if (graph.tracker === "jira") {
    // OpenCode puts no helper on PATH; the jira skill runs it from the data root.
    const read =
      kind === "opencode"
        ? `\`"$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/jira/jira.sh" issue get ${issue.ref}\` (\`skill({ name: "jira-jira" })\`)`
        : `\`jira.sh issue get ${issue.ref}\` (the toolu jira skill)`;
    return {
      read,
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
  const lines = trackerLines(graph, issue, kind);
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
    JOB: `bun "${join(SCRIPTS_DIR, "job.ts")}"`,
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

function promptCommand(
  key: string,
  paths: { brief: string; status: string },
  base: string,
  resuming: boolean,
  resumed: boolean,
): string[] {
  const prompt = (resuming ? RESUME_PROMPT : START_PROMPT)
    .replace("{brief}", paths.brief)
    .replace("{status}", paths.status)
    .replace("{base}", base)
    .concat(resumed ? " Your previous conversation for this issue is loaded above." : "");
  return [
    "agent",
    "prompt",
    key,
    prompt,
    "--wait",
    "--until",
    "working",
    "--until",
    "blocked",
    "--timeout",
    "60000",
  ];
}

async function initializeAttempt(
  issue: GraphIssue,
  record: Record<string, unknown>,
  host: HostKind,
  at: number,
  state: string,
  reservation: Awaited<ReturnType<typeof launchReservation>>,
): Promise<void> {
  for (const k of ["ref", "key", "repo", "number", "url", "title", "branch"] as const)
    record[k] = issue[k];
  record.kind = host;
  record.prompt_state = "pending";
  record.attempt_started_at = new Date(at).toISOString();
  await persistAttempt(state, issue.key, record, reservation.root, reservation.lease, "starting");
}

function finishRecord(
  record: Record<string, unknown>,
  values: {
    host: Resolved;
    status: string;
    brief: string;
    started: boolean;
    prompted: boolean;
    launches: number;
    outcome: string;
  },
): void {
  const now = new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
  Object.assign(record, {
    model: values.host.model ?? null,
    effort: values.host.effort ?? null,
    stage: "running",
    status_file: values.status,
    brief: values.brief,
    launched_at: record.launched_at ?? now,
    last_launch: values.prompted ? now : record.last_launch,
    launches: values.started ? values.launches + 1 : values.launches,
    prompt_state: "acknowledged",
    lifecycle_outcome: values.outcome,
  });
}

function requireRecoverableStage(record: Record<string, unknown>, opts: LaunchOpts): void {
  if (["cleaning", "cleanup-incomplete"].includes(String(record.stage)))
    throw new CommandError(
      "cleanup is incomplete; reconcile with finish-issue before launching again",
    );
  if (record.stage === "replacing" && !opts.replace)
    throw new CommandError("replacement is incomplete; retry explicitly with --replace");
  if (["starting", "uncertain"].includes(String(record.stage)) && !opts.reprompt && !opts.replace)
    throw new CommandError(
      "previous launch/prompt outcome is uncertain; inspect the recorded pane/session before explicit --reprompt or --replace",
    );
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
  const record = loadLaunchRecord(join(state, "issues", `${issue.key}.json`));
  const resuming = Object.keys(record).length > 0 || issue.status === "in_flight";
  const route = loadLaunchRoute(join(state, "routes", `${issue.key}.json`));
  const host = resolveHost(opts, route);
  // Before any clone, gh, herdr or state write.
  if (host.kind === "opencode") await checkOpencode(host.model, dry);
  const launches = typeof record.launches === "number" ? record.launches : 0;
  const previousKind = typeof record.kind === "string" ? record.kind : undefined;
  const attemptStarted = Date.now();
  requireRecoverableStage(record, opts);
  if (previousKind !== undefined && previousKind !== host.kind && !opts.replace)
    throw new CommandError("host change requires explicit --replace");
  if (!dry && launches === 0 && !opts.force) await guardBudget();
  const reservation = dry
    ? null
    : await launchReservation(
        state,
        issue.key,
        host.kind,
        graph.max_parallel ?? 3,
        record,
        opts.replace,
      );
  try {
    if (reservation)
      await initializeAttempt(issue, record, host.kind, attemptStarted, state, reservation);
    const [checkout, base] = await prepareCheckout(graph, issue, dry, log);
    const wt = await ensureWorktree(checkout, issue, base, dry, log);
    if (host.kind === "opencode") log.push(await excludeForWorker(checkout, dry));
    if (reservation) {
      Object.assign(record, { ...wt, checkout, base, agent: issue.key });
      bindWorktree(reservation.root, wt.worktree, issue.key, state);
      await persistAttempt(
        state,
        issue.key,
        record,
        reservation.root,
        reservation.lease,
        "starting",
      );
    }
    const plan: AgentPlan = {
      host,
      bypass: !opts.safe,
      permissionMode: opts.permissionMode,
      previousKind,
      launches,
      worktree: wt.worktree,
      ...(typeof record.session_id === "string" ? { sessionId: record.session_id } : {}),
      ...(typeof record.session_started_after === "number"
        ? { sessionStartedAfter: record.session_started_after }
        : {}),
    };
    const [started, resumed] = await ensureAgent(issue.key, wt.pane_id, plan, opts, log);
    if (!dry && started && !resumed) {
      record.session_started_after = attemptStarted;
      record.session_id = await captureSession(host.kind, wt.worktree, attemptStarted);
    }
    const paths = {
      worktree: wt.worktree,
      status: join(state, "status", `${issue.key}.json`),
      brief: join(state, "briefs", `${issue.key}.md`),
    };
    const brief = renderBrief(graph, issue, paths, base, host.kind);
    const promptCmd = promptCommand(issue.key, paths, base, resuming, resumed);
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
    if (reservation) {
      Object.assign(record, {
        brief: paths.brief,
        status_file: paths.status,
        prompt_state: started || opts.reprompt ? "submitting" : "retained",
      });
      await persistAttempt(
        state,
        issue.key,
        record,
        reservation.root,
        reservation.lease,
        "starting",
      );
    }
    if (started || opts.reprompt) await herdr(promptCmd);
    if (started && !resumed) {
      record.session_id = await awaitSession(host.kind, wt.worktree, attemptStarted);
    }
    const lifecycleOutcome = await acknowledgedOutcome(
      issue.key,
      { kind: host.kind, pane: wt.pane_id, cwd: wt.worktree },
      resumed ? "resumed" : started ? "started" : "retained",
    );
    record.bypass = !opts.safe;
    finishRecord(record, {
      host,
      status: paths.status,
      brief: paths.brief,
      started,
      prompted: started || opts.reprompt,
      launches,
      outcome: lifecycleOutcome,
    });
    if (reservation)
      await persistAttempt(
        state,
        issue.key,
        record,
        reservation.root,
        reservation.lease,
        "running",
      );
    return { issue: issue.ref, commands: log, ...record };
  } catch (error) {
    if (reservation) {
      Object.assign(record, failureDetails(error));
      await persistAttempt(
        state,
        issue.key,
        record,
        reservation.root,
        reservation.lease,
        "uncertain",
      );
    }
    throw error;
  }
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
    const issue = findIssue(graph, issueRef);
    result = opts.dryRun
      ? await launch(graph, issue, opts)
      : await withLaunchOwnership(graph.state_dir, issue.key, () => launch(graph, issue, opts));
  } catch (err) {
    if (err instanceof CommandError) {
      process.stderr.write(JSON.stringify({ issue: issueRef, error: String(err) }) + "\n");
      process.exit(commandExitCode(err));
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
