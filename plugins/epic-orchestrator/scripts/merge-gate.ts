/** Verify a sub-issue PR against the epic merge policy and optionally merge it. */

import { join } from "node:path";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import { ghJson, parseRef, readJson, run, writeJson } from "./common.ts";
import { detectTracker, makeTracker } from "./trackers/index.ts";

const STAGE_AFTER: Record<string, string> = {
  merge: "running",
  wait: "awaiting_merge",
  rebase: "running",
  fix: "running",
  merged: "running",
  closed: "running",
};

const FAIL = new Set([
  "FAILURE",
  "TIMED_OUT",
  "CANCELLED",
  "ACTION_REQUIRED",
  "STARTUP_FAILURE",
  "ERROR",
]);
const PASS = new Set(["SUCCESS", "NEUTRAL", "SKIPPED"]);

export const PROTECTION =
  /protected branch|branch policy|required status|review is required|approving review|not mergeable: the base branch|merge requirements|--admin/i;

type MergeAttempt = {
  exitCode: number;
  stdout: string;
  stderr: string;
  timedOut: boolean;
  cancelled: boolean;
  truncated: boolean;
};

export function mergeAttemptOutcome(
  result: MergeAttempt,
): "success" | "admin" | "failed" | "uncertain" {
  if (result.timedOut || result.cancelled || result.truncated) return "uncertain";
  if (result.exitCode === 0) return "success";
  return PROTECTION.test(result.stderr + result.stdout) ? "admin" : "failed";
}

function mergeFailure(result: MergeAttempt, admin: boolean): Record<string, unknown> {
  const outcome = mergeAttemptOutcome(result);
  const detail = (result.stderr || result.stdout).trim();
  const nativeCode = result.timedOut
    ? "timeout"
    : result.cancelled
      ? "cancelled"
      : result.truncated
        ? "output_limit"
        : "exit";
  return {
    merged: false,
    ...(admin ? { admin_used: true } : {}),
    native_exit_code: result.exitCode,
    native_error_code: nativeCode,
    uncertain: outcome === "uncertain",
    error:
      detail ||
      (result.timedOut
        ? "merge command timed out"
        : result.cancelled
          ? "merge command was cancelled"
          : "merge command output exceeded its limit"),
  };
}

const THREADS = `query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){pullRequest(number:$n){
reviewThreads(first:100){nodes{isResolved isOutdated}}}}}`;

type CheckItem = {
  name?: string;
  context?: string;
  __typename?: string;
  state?: string;
  status?: string;
  conclusion?: string;
};

export function checkBuckets(rollup: CheckItem[]): Record<string, string[]> {
  const out: Record<string, string[]> = { pass: [], fail: [], pending: [] };
  for (const c of rollup) {
    const name = c.name || c.context || "?";
    let bucket: string;
    if (c.__typename === "StatusContext") {
      const state = c.state;
      bucket = state === "SUCCESS" ? "pass" : state && FAIL.has(state) ? "fail" : "pending";
    } else if (c.status !== "COMPLETED") {
      bucket = "pending";
    } else {
      bucket = c.conclusion && PASS.has(c.conclusion) ? "pass" : "fail";
    }
    const list = out[bucket];
    if (list) list.push(name);
  }
  return out;
}

async function assess(
  owner: string,
  repo: string,
  number: number,
): Promise<Record<string, unknown>> {
  const pr = (await ghJson([
    "pr",
    "view",
    String(number),
    "-R",
    `${owner}/${repo}`,
    "--json",
    "state,isDraft,mergeable,headRefOid,headRefName,baseRefName,statusCheckRollup,url,autoMergeRequest",
  ])) as {
    state: string;
    isDraft: boolean;
    mergeable: string;
    headRefOid: string;
    headRefName: string;
    baseRefName: string;
    statusCheckRollup: CheckItem[] | null;
    url: string;
    autoMergeRequest: unknown;
  };
  const result: Record<string, unknown> = {
    pr: `${owner}/${repo}#${number}`,
    url: pr.url,
    head: pr.headRefOid,
    reasons: [] as string[],
    auto_merge_armed: pr.autoMergeRequest !== null && pr.autoMergeRequest !== undefined,
  };
  if (pr.state !== "OPEN") {
    return { ...result, verdict: pr.state.toLowerCase() };
  }
  const reasons = result.reasons as string[];
  const behindResp = (await ghJson([
    "api",
    `repos/${owner}/${repo}/compare/${pr.baseRefName}...${pr.headRefOid}`,
    "--jq",
    "{b: .behind_by}",
  ])) as { b: number };
  const behind = behindResp.b;
  const checks = checkBuckets(pr.statusCheckRollup ?? []);
  if (!pr.statusCheckRollup) {
    const workflows = (await ghJson([
      "api",
      `repos/${owner}/${repo}/actions/workflows`,
      "--jq",
      "{n: .total_count}",
    ])) as { n: number };
    if (workflows.n) checks.pending?.push("(no checks reported yet)");
  }
  const threads =
    ((await ghJson([
      "api",
      "graphql",
      "-f",
      `query=${THREADS}`,
      "-F",
      `o=${owner}`,
      "-F",
      `r=${repo}`,
      "-F",
      `n=${number}`,
      "--jq",
      ".data.repository.pullRequest.reviewThreads.nodes",
    ])) as { isResolved: boolean; isOutdated: boolean }[] | null) ?? [];
  const unresolved = threads.filter((t) => !t.isResolved).length;
  Object.assign(result, {
    behind_by: behind,
    mergeable: pr.mergeable,
    checks,
    unresolved_threads: unresolved,
    draft: pr.isDraft,
    base: pr.baseRefName,
  });
  if (pr.mergeable === "CONFLICTING" || behind) {
    reasons.push(
      pr.mergeable === "CONFLICTING" ? "conflicting with base" : `behind base by ${behind}`,
    );
    return { ...result, verdict: "rebase" };
  }
  if (checks.fail?.length || unresolved || pr.isDraft) {
    if (checks.fail?.length) reasons.push(`failing: ${checks.fail.join(", ")}`);
    if (unresolved) reasons.push(`${unresolved} unresolved review threads`);
    if (pr.isDraft) reasons.push("draft");
    return { ...result, verdict: "fix" };
  }
  if ((checks.pending?.length ?? 0) > 0 || pr.mergeable === "UNKNOWN") {
    reasons.push("pending: " + ((checks.pending ?? []).join(", ") || "mergeability not computed"));
    return { ...result, verdict: "wait" };
  }
  return { ...result, verdict: "merge" };
}

async function mergeMethod(
  owner: string,
  repo: string,
  wanted: string | undefined,
): Promise<string> {
  if (wanted) return wanted;
  const s = (await ghJson([
    "repo",
    "view",
    `${owner}/${repo}`,
    "--json",
    "squashMergeAllowed,mergeCommitAllowed,rebaseMergeAllowed",
  ])) as {
    squashMergeAllowed: boolean;
    mergeCommitAllowed: boolean;
    rebaseMergeAllowed: boolean;
  };
  return s.squashMergeAllowed ? "squash" : s.mergeCommitAllowed ? "merge" : "rebase";
}

async function doMerge(
  owner: string,
  repo: string,
  number: number,
  head: string,
  method: string,
): Promise<Record<string, unknown>> {
  const base = [
    "gh",
    "pr",
    "merge",
    String(number),
    "-R",
    `${owner}/${repo}`,
    `--${method}`,
    "--delete-branch",
    "--match-head-commit",
    head,
  ];
  const first = await runCommand(base, { timeoutMs: 120_000, maxOutputBytes: 65_536 });
  let admin = false;
  const firstOutcome = mergeAttemptOutcome(first);
  if (firstOutcome === "uncertain" || firstOutcome === "failed") return mergeFailure(first, false);
  if (firstOutcome === "admin") {
    const second = await runCommand([...base, "--admin"], {
      timeoutMs: 120_000,
      maxOutputBytes: 65_536,
    });
    if (mergeAttemptOutcome(second) !== "success") return mergeFailure(second, true);
    admin = true;
  }
  for (let i = 0; i < 15; i++) {
    const state = (
      await run([
        "gh",
        "pr",
        "view",
        String(number),
        "-R",
        `${owner}/${repo}`,
        "--json",
        "state",
        "-q",
        ".state",
      ])
    ).trim();
    if (state === "MERGED") return { merged: true, admin_used: admin, method };
    await Bun.sleep(2000);
  }
  return {
    merged: false,
    admin_used: admin,
    error: "merge command succeeded but PR is not MERGED after 30s",
  };
}

/** Arm GitHub auto-merge pinned to the verified head: GitHub merges the moment
 * the pending checks pass, with no orchestrator polling. A later push does
 * not match the pinned SHA, and the gate disarms on `rebase`/`fix`. */
export function autoMergeArgs(
  owner: string,
  repo: string,
  number: number,
  head: string,
  method: string,
): string[] {
  return [
    "gh",
    "pr",
    "merge",
    String(number),
    "-R",
    `${owner}/${repo}`,
    "--auto",
    `--${method}`,
    "--delete-branch",
    "--match-head-commit",
    head,
  ];
}

/** Arm only a PR that is waiting on checks alone; disarm whenever the worker
 * is about to push (rebase/fix), so no unverified head can merge itself. */
export function autoMergeAction(
  verdict: string,
  armed: boolean,
  wanted: boolean,
): "arm" | "disarm" | null {
  if (armed && (verdict === "rebase" || verdict === "fix")) return "disarm";
  if (wanted && !armed && verdict === "wait") return "arm";
  return null;
}

/** Auto-merge state after this gate run, for the issue record: `armed`,
 * `off`, or `unavailable` (arming was tried and the repo refused it). */
export function autoMergeState(r: {
  auto_merge_armed?: unknown;
  auto_merge?: unknown;
  auto_merge_disarmed?: unknown;
}): "armed" | "off" | "unavailable" {
  if (r.auto_merge_disarmed === true) return "off";
  if (r.auto_merge === true) return "armed";
  if (r.auto_merge === false) return "unavailable";
  return r.auto_merge_armed === true ? "armed" : "off";
}

async function armAutoMerge(
  owner: string,
  repo: string,
  number: number,
  head: string,
  method: string,
): Promise<Record<string, unknown>> {
  try {
    await run(autoMergeArgs(owner, repo, number, head, method), { write: true });
    return { auto_merge: true, method };
  } catch (err) {
    // Repos with auto-merge off: the watcher's recheck merges instead.
    return {
      auto_merge: false,
      auto_merge_error: err instanceof Error ? err.message : String(err),
    };
  }
}

async function disarmAutoMerge(owner: string, repo: string, number: number): Promise<void> {
  await run(["gh", "pr", "merge", String(number), "-R", `${owner}/${repo}`, "--disable-auto"], {
    write: true,
  });
}

type GraphLite = {
  tracker?: string;
  default_repo?: string | null;
  epic?: { ref: string };
  issues?: { ref: string; key: string; title: string }[];
};

/** Close the delivered item and tick the epic through the epic's tracker
 * (read from the saved graph, else detected from the epic ref). */
async function settleIssue(
  issue: string,
  prUrl: string,
  epic: string | undefined,
  graph: GraphLite,
): Promise<{ issue: string; epic_ticked?: boolean }> {
  const epicRef = epic ?? graph.epic?.ref;
  const kind = detectTracker(epicRef ?? issue, graph.tracker);
  const tracker = makeTracker(kind, epicRef ?? issue, graph.default_repo ?? "");
  const note = `Delivered in ${prUrl}` + (epicRef ? ` (epic ${epicRef}).` : ".");
  const closed = await tracker.closeIssue(issue, note);
  if (!epicRef) return { issue: closed };
  let title = graph.issues?.find((i) => i.ref === issue || i.key === issue)?.title;
  if (title === undefined && kind === "github") {
    const [o, r, n] = parseRef(issue);
    title = (
      (await ghJson(["api", `repos/${o}/${r}/issues/${n}`, "--jq", "{t: .title}"])) as { t: string }
    ).t;
  }
  return { issue: closed, epic_ticked: await tracker.tickEpic(issue, title ?? "") };
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let prRef: string | undefined;
  let issue: string | undefined;
  let epic: string | undefined;
  let doMergeFlag = false;
  let autoFlag = false;
  let method: string | undefined;
  let stateDir: string | undefined;
  let key: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--pr") prRef = argv[++i];
    else if (a === "--issue") issue = argv[++i];
    else if (a === "--epic") epic = argv[++i];
    else if (a === "--merge") doMergeFlag = true;
    else if (a === "--auto") autoFlag = true;
    else if (a === "--method") method = argv[++i];
    else if (a === "--state-dir") stateDir = argv[++i];
    else if (a === "--key") key = argv[++i];
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!prRef) throw new Error("--pr is required");
  const [owner, repo, number] = parseRef(prRef);
  const result = await assess(owner, repo, number);
  if (doMergeFlag && result.verdict === "merge") {
    Object.assign(
      result,
      await doMerge(
        owner,
        repo,
        number,
        String(result.head),
        await mergeMethod(owner, repo, method),
      ),
    );
  }
  const action = autoMergeAction(
    String(result.verdict),
    result.auto_merge_armed === true,
    autoFlag,
  );
  if (action === "arm") {
    const m = await mergeMethod(owner, repo, method);
    Object.assign(result, await armAutoMerge(owner, repo, number, String(result.head), m));
  } else if (action === "disarm") {
    await disarmAutoMerge(owner, repo, number);
    result.auto_merge_disarmed = true;
  }
  if ((result.merged || result.verdict === "merged") && issue) {
    const graph = stateDir ? readJson<GraphLite>(join(stateDir, "graph.json"), {}) : {};
    Object.assign(result, await settleIssue(issue, String(result.url), epic, graph));
  }
  if (stateDir && key) {
    const recPath = join(stateDir, "issues", `${key}.json`);
    const rec = readJson<Record<string, unknown>>(recPath, {});
    const verdict = String(result.verdict);
    rec.stage = STAGE_AFTER[verdict] ?? "running";
    rec.last_gate = {
      verdict: result.verdict,
      reasons: result.reasons,
      head: result.head,
      merged: result.merged,
      admin_used: result.admin_used,
      uncertain: result.uncertain,
      native_exit_code: result.native_exit_code,
      native_error_code: result.native_error_code,
      error: result.error,
      auto_merge: autoMergeState(result),
    };
    await writeJson(recPath, rec);
  }
  process.stdout.write(JSON.stringify(result, null, 2) + "\n");
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
