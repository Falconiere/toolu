/** Verify a sub-issue PR against the epic merge policy and optionally merge it. */

import { join } from "node:path";
import { ghJson, parseRef, readJson, run, writeJson } from "./common.ts";

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
    "state,isDraft,mergeable,headRefOid,headRefName,baseRefName,statusCheckRollup,url",
  ])) as {
    state: string;
    isDraft: boolean;
    mergeable: string;
    headRefOid: string;
    headRefName: string;
    baseRefName: string;
    statusCheckRollup: CheckItem[] | null;
    url: string;
  };
  const result: Record<string, unknown> = {
    pr: `${owner}/${repo}#${number}`,
    url: pr.url,
    head: pr.headRefOid,
    reasons: [] as string[],
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
  const first = Bun.spawnSync(base, { stdout: "pipe", stderr: "pipe" });
  let admin = false;
  if (first.exitCode !== 0) {
    const errText = first.stderr.toString() + first.stdout.toString();
    if (!PROTECTION.test(errText)) {
      return { merged: false, error: errText.trim() };
    }
    const second = Bun.spawnSync([...base, "--admin"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    if (second.exitCode !== 0) {
      return {
        merged: false,
        admin_used: true,
        error: (second.stderr.toString() || second.stdout.toString()).trim(),
      };
    }
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

async function closeIssue(issue: string, prUrl: string, epic: string | undefined): Promise<string> {
  const [o, r, n] = parseRef(issue);
  for (let i = 0; i < 5; i++) {
    const state = (
      await run([
        "gh",
        "issue",
        "view",
        String(n),
        "-R",
        `${o}/${r}`,
        "--json",
        "state",
        "-q",
        ".state",
      ])
    ).trim();
    if (state === "CLOSED") return "closed-by-pr";
    await Bun.sleep(3000);
  }
  const note = `Delivered in ${prUrl}` + (epic ? ` (epic ${epic}).` : ".");
  await run([
    "gh",
    "issue",
    "close",
    String(n),
    "-R",
    `${o}/${r}`,
    "--reason",
    "completed",
    "--comment",
    note,
  ]);
  return "closed-manually";
}

function words(text: string): string[] {
  return (
    text
      .replace(/^\s*\[[^\]]*\]\s*/, "")
      .toLowerCase()
      .match(/[a-z0-9]+/g) ?? []
  );
}

export function tickBody(
  body: string,
  epicRepo: [string, string],
  issue: string,
  title = "",
): string {
  const [io, ir, inum] = parseRef(issue);
  const full = [`https://github.com/${io}/${ir}/issues/${inum}`, `${io}/${ir}#${inum}`];
  const task = new RegExp(
    `^(\\s*[-*]\\s*)\\[ \\](\\s*)(${full.map((s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})(?!\\d)`,
    "i",
  );
  const sameRepo =
    io.toLowerCase() === epicRepo[0].toLowerCase() &&
    ir.toLowerCase() === epicRepo[1].toLowerCase();
  const bare = new RegExp(`^(\\s*[-*]\\s*)\\[ \\](\\s*)(#${inum})(?!\\d)(.*)$`);
  const head = words(title).slice(0, 3);
  const lines = body.split("\n");
  for (let idx = 0; idx < lines.length; idx++) {
    const line = lines[idx];
    if (line === undefined) continue;
    if (task.test(line)) {
      lines[idx] = line.replace(task, "$1[x]$2$3");
    } else if (sameRepo) {
      const m = bare.exec(line);
      if (m) {
        const rest = words(m[4] ?? "");
        if (
          rest.length === 0 ||
          (head.length > 0 && rest.slice(0, head.length).join() === head.join())
        ) {
          lines[idx] = line.replace(bare, "$1[x]$2$3$4");
        }
      }
    }
  }
  return lines.join("\n");
}

async function tickEpic(epic: string, issue: string): Promise<boolean> {
  const [eo, er, en] = parseRef(epic);
  const [io, ir, inum] = parseRef(issue);
  const titleResp = (await ghJson([
    "api",
    `repos/${io}/${ir}/issues/${inum}`,
    "--jq",
    "{t: .title}",
  ])) as { t: string };
  const bodyResp = (await ghJson([
    "api",
    `repos/${eo}/${er}/issues/${en}`,
    "--jq",
    "{b: .body}",
  ])) as { b: string | null };
  const body = bodyResp.b ?? "";
  const next = tickBody(body, [eo, er], issue, titleResp.t);
  if (next === body) return false;
  await run(["gh", "api", "-X", "PATCH", `repos/${eo}/${er}/issues/${en}`, "-f", `body=${next}`]);
  return true;
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let prRef: string | undefined;
  let issue: string | undefined;
  let epic: string | undefined;
  let doMergeFlag = false;
  let method: string | undefined;
  let stateDir: string | undefined;
  let key: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--pr") prRef = argv[++i];
    else if (a === "--issue") issue = argv[++i];
    else if (a === "--epic") epic = argv[++i];
    else if (a === "--merge") doMergeFlag = true;
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
  if ((result.merged || result.verdict === "merged") && issue) {
    result.issue = await closeIssue(issue, String(result.url), epic);
    if (epic) result.epic_ticked = await tickEpic(epic, issue);
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
