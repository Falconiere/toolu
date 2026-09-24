/** Build an epic's sub-issue dependency graph and pick the next launch batch. */

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { readdirSync } from "node:fs";
import {
  CommandError,
  ghJson,
  issueKey,
  parseRef,
  readJson,
  slugify,
  stateDirFor,
} from "./common.ts";
import { resolveCheckouts } from "./checkouts.ts";

const DEP_LINE = /\b(?:blocked by|depends on)\b[^\n]*/gi;
/** URL, owner/repo#N, or bare #N (default repo supplied by parseRef). */
export const ISSUE_REF =
  /https:\/\/github\.com\/[^\s)]+\/issues\/\d+|[\w.-]+\/[\w.-]+#\d+|(?<![\w/])#\d+/g;
const CLOSING_PRS = `query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){issue(number:$n){
closedByPullRequestsReferences(first:10,includeClosedPrs:true){nodes{number state url headRefName}}}}}`;

type IssueItem = {
  repository_url: string;
  number: number;
  title: string;
  state: string;
  html_url: string;
};

type SubIssue = {
  ref: string;
  title: string;
  state: string;
  url: string;
};

type PrNode = { number: number; state: string; url: string; headRefName: string };

export type GraphIssue = SubIssue & {
  repo: string;
  number: number;
  blockers: Record<string, string>;
  prs: PrNode[];
  deps_source: string;
  key: string;
  status?: string;
  open_blockers?: string[];
  wave?: number | null;
  unblocks?: number;
  chain?: number;
  checkout?: string | null;
  branch?: string;
  stage?: string | null;
};

function refOf(item: IssueItem): string {
  const parts = item.repository_url.replace(/\/$/, "").split("/");
  const owner = parts[parts.length - 2];
  const repo = parts[parts.length - 1];
  if (!owner || !repo) throw new Error(`bad repository_url: ${item.repository_url}`);
  return `${owner}/${repo}#${item.number}`;
}

async function fetchSubIssues(
  owner: string,
  repo: string,
  number: number,
  body: string,
): Promise<SubIssue[]> {
  const items = (await ghJson([
    "api",
    "--paginate",
    "--slurp",
    `repos/${owner}/${repo}/issues/${number}/sub_issues?per_page=100`,
  ])) as IssueItem[][] | null;
  const subs = (items ?? []).flat();
  if (subs.length > 0) {
    return subs.map((i) => ({
      ref: refOf(i),
      title: i.title,
      state: i.state,
      url: i.html_url,
    }));
  }
  const refs: string[] = [];
  for (const line of body.split("\n")) {
    if (/^\s*[-*]\s*\[[ xX]\]/.test(line)) {
      // Same shapes as DEP_LINE extraction: URL, owner/repo#N, and bare #N.
      const re = new RegExp(ISSUE_REF.source, ISSUE_REF.flags);
      let m: RegExpExecArray | null;
      while ((m = re.exec(line)) !== null) {
        const [o, r, n] = parseRef(m[0], `${owner}/${repo}`);
        refs.push(`${o}/${r}#${n}`);
      }
    }
  }
  const out: SubIssue[] = [];
  for (const ref of Array.from(new Set(refs))) {
    const [o, r, n] = parseRef(ref);
    const i = (await ghJson([`api`, `repos/${o}/${r}/issues/${n}`])) as IssueItem;
    out.push({ ref, title: i.title, state: i.state, url: i.html_url });
  }
  return out;
}

async function fetchDetails(sub: SubIssue): Promise<Omit<GraphIssue, "key">> {
  const [owner, repo, number] = parseRef(sub.ref);
  const blockers: Record<string, string> = {};
  let depsApi = true;
  try {
    const blocked =
      ((await ghJson([
        "api",
        `repos/${owner}/${repo}/issues/${number}/dependencies/blocked_by`,
      ])) as IssueItem[] | null) ?? [];
    for (const b of blocked) {
      blockers[refOf(b)] = b.state;
    }
  } catch (err) {
    if (err instanceof CommandError) depsApi = false;
    else throw err;
  }
  const bodyResp = (await ghJson([
    "api",
    `repos/${owner}/${repo}/issues/${number}`,
    "--jq",
    "{body: .body}",
  ])) as { body: string | null };
  const body = bodyResp.body ?? "";
  for (const line of body.match(DEP_LINE) ?? []) {
    const re = new RegExp(ISSUE_REF.source, ISSUE_REF.flags);
    let m: RegExpExecArray | null;
    while ((m = re.exec(line)) !== null) {
      const [o, r, n] = parseRef(m[0], `${owner}/${repo}`);
      const ref = `${o}/${r}#${n}`;
      if (!(ref in blockers) && ref !== sub.ref) {
        const st = (await ghJson([
          "api",
          `repos/${o}/${r}/issues/${n}`,
          "--jq",
          "{s: .state}",
        ])) as { s: string };
        blockers[ref] = st.s;
      }
    }
  }
  const prs =
    ((await ghJson([
      "api",
      "graphql",
      "-f",
      `query=${CLOSING_PRS}`,
      "-F",
      `o=${owner}`,
      "-F",
      `r=${repo}`,
      "-F",
      `n=${number}`,
      "--jq",
      ".data.repository.issue.closedByPullRequestsReferences.nodes",
    ])) as PrNode[] | null) ?? [];
  return {
    ...sub,
    repo: `${owner}/${repo}`,
    number,
    blockers,
    prs,
    deps_source: depsApi ? "native+text" : "text",
  };
}

export function computeLevels(
  issues: Record<string, GraphIssue>,
): [Record<string, number>, string[]] {
  const openRefs = new Set(
    Object.entries(issues)
      .filter(([, i]) => i.state === "open")
      .map(([r]) => r),
  );
  const deps: Record<string, Set<string>> = {};
  for (const r of openRefs) {
    const issue = issues[r];
    if (!issue) continue;
    deps[r] = new Set(
      Object.entries(issue.blockers)
        .filter(([b, s]) => s === "open" && openRefs.has(b))
        .map(([b]) => b),
    );
  }
  const levels: Record<string, number> = {};
  const pending = new Set(openRefs);
  while (pending.size > 0) {
    const progress = [...pending].filter((r) => {
      const d = deps[r];
      return d !== undefined && [...d].every((b) => b in levels);
    });
    if (progress.length === 0) {
      // Refuse all waves until the cycle is fixed — partial levels would
      // schedule acyclic siblings while the graph is still unschedulable.
      return [{}, [...pending].sort()];
    }
    for (const r of progress) {
      const d = deps[r] ?? new Set<string>();
      let maxDep = 0;
      for (const dep of d) {
        const lv = levels[dep];
        if (lv !== undefined && lv > maxDep) maxDep = lv;
      }
      levels[r] = 1 + maxDep;
    }
    for (const r of progress) pending.delete(r);
  }
  return [levels, []];
}

export function downstreamCounts(issues: Record<string, GraphIssue>): Record<string, number> {
  const dependents: Record<string, Set<string>> = {};
  for (const r of Object.keys(issues)) dependents[r] = new Set();
  for (const [r, i] of Object.entries(issues)) {
    for (const b of Object.keys(i.blockers)) {
      if (b in dependents && i.state === "open") {
        dependents[b]?.add(r);
      }
    }
  }

  function reach(r: string, seen: Set<string>): Set<string> {
    const deps = dependents[r] ?? new Set();
    for (const d of deps) {
      if (!seen.has(d)) {
        seen.add(d);
        reach(d, seen);
      }
    }
    return seen;
  }

  const out: Record<string, number> = {};
  for (const r of Object.keys(issues)) {
    out[r] = reach(r, new Set()).size;
  }
  return out;
}

export function chainLengths(issues: Record<string, GraphIssue>): Record<string, number> {
  const dependents: Record<string, Set<string>> = {};
  for (const r of Object.keys(issues)) dependents[r] = new Set();
  for (const [r, i] of Object.entries(issues)) {
    if (i.state === "open") {
      for (const b of Object.keys(i.blockers)) {
        if (b in dependents) dependents[b]?.add(r);
      }
    }
  }
  const memo: Record<string, number> = {};

  function depth(r: string, trail: ReadonlySet<string>): number {
    if (!(r in memo)) {
      const deps = dependents[r] ?? new Set();
      let maxNext = 0;
      for (const d of deps) {
        if (!trail.has(d)) {
          const next = depth(d, new Set([...trail, r]));
          if (next > maxNext) maxNext = next;
        }
      }
      memo[r] = 1 + maxNext;
    }
    const cached = memo[r];
    return cached ?? 1;
  }

  const out: Record<string, number> = {};
  for (const r of Object.keys(issues)) {
    out[r] = depth(r, new Set());
  }
  return out;
}

export function pickBatch(
  ready: string[],
  issues: Record<string, GraphIssue>,
  inFlight: string[],
  slots: number,
): string[] {
  const batch: string[] = [];
  const load: Record<string, number> = {};
  for (const r of inFlight) {
    const issue = issues[r];
    if (!issue) continue;
    load[issue.repo] = (load[issue.repo] ?? 0) + 1;
  }
  const pool = [...ready];
  const sortKey = (r: string): [number, number, number, string, number] => {
    const i = issues[r];
    if (!i) return [0, 0, 0, "", 0];
    return [-(i.chain ?? 0), -(i.unblocks ?? 0), load[i.repo] ?? 0, i.repo, i.number];
  };
  const keyLess = (
    a: [number, number, number, string, number],
    b: [number, number, number, string, number],
  ): boolean =>
    a[0] < b[0] ||
    (a[0] === b[0] && a[1] < b[1]) ||
    (a[0] === b[0] && a[1] === b[1] && a[2] < b[2]) ||
    (a[0] === b[0] && a[1] === b[1] && a[2] === b[2] && a[3] < b[3]) ||
    (a[0] === b[0] && a[1] === b[1] && a[2] === b[2] && a[3] === b[3] && a[4] < b[4]);
  while (pool.length > 0 && batch.length < slots) {
    let bestIdx = 0;
    let bestKey = sortKey(pool[0] ?? "");
    for (let i = 1; i < pool.length; i++) {
      const cand = pool[i];
      if (cand === undefined) continue;
      const ck = sortKey(cand);
      if (keyLess(ck, bestKey)) {
        bestIdx = i;
        bestKey = ck;
      }
    }
    const best = pool[bestIdx];
    if (best === undefined) break;
    batch.push(best);
    pool.splice(bestIdx, 1);
    const chosen = issues[best];
    if (chosen) load[chosen.repo] = (load[chosen.repo] ?? 0) + 1;
  }
  return batch;
}

export function classify(
  issue: GraphIssue,
  inEpic: Set<string>,
  launched: Record<string, { stage?: string }>,
): string {
  if (issue.state === "closed") return "done";
  const stage = launched[issue.key]?.stage;
  const live = stage !== undefined && stage !== "merged" && stage !== "abandoned";
  if (live || issue.prs.some((p) => p.state === "OPEN")) return "in_flight";
  const openBlockers = Object.entries(issue.blockers)
    .filter(([, s]) => s === "open")
    .map(([b]) => b);
  if (openBlockers.some((b) => !inEpic.has(b))) return "external_blocked";
  return openBlockers.length > 0 ? "blocked" : "ready";
}

async function build(epicRef: string, maxParallel: number): Promise<Record<string, unknown>> {
  const [owner, repo, number] = parseRef(epicRef);
  const epic = (await ghJson([`api`, `repos/${owner}/${repo}/issues/${number}`])) as {
    title: string;
    state: string;
    html_url: string;
    body?: string | null;
  };
  const subs = await fetchSubIssues(owner, repo, number, epic.body ?? "");
  const details = await Promise.all(subs.map(fetchDetails));
  const stateDir = stateDirFor(owner, repo, number);
  const launched: Record<string, Record<string, unknown>> = {};
  try {
    for (const name of readdirSync(join(stateDir, "issues"))) {
      if (!name.endsWith(".json")) continue;
      const stem = name.slice(0, -5);
      launched[stem] = readJson(join(stateDir, "issues", name), {});
    }
  } catch {
    // no state dir yet
  }
  const issues: Record<string, GraphIssue> = {};
  for (const d of details) {
    issues[d.ref] = { ...d, key: issueKey(d.repo, d.number) };
  }
  const [levels, cycle] = computeLevels(issues);
  const downstream = downstreamCounts(issues);
  const chains = chainLengths(issues);
  const checkouts = await resolveCheckouts(
    [...new Set([...Object.values(issues).map((i) => i.repo), `${owner}/${repo}`])].sort(),
  );
  for (const [ref, i] of Object.entries(issues)) {
    i.status = classify(i, new Set(Object.keys(issues)), launched);
    i.open_blockers = Object.entries(i.blockers)
      .filter(([, s]) => s === "open")
      .map(([b]) => b)
      .sort();
    i.wave = levels[ref] ?? null;
    i.unblocks = downstream[ref] ?? 0;
    i.chain = i.state === "open" ? (chains[ref] ?? 0) : 0;
    i.checkout = checkouts[i.repo] ?? null;
    const rec = launched[i.key] ?? {};
    const branch = typeof rec.branch === "string" ? rec.branch : null;
    i.branch = branch ?? `feat/${i.number}-${slugify(i.title)}`;
    i.stage = typeof rec.stage === "string" ? rec.stage : null;
  }
  const inFlight = Object.entries(issues)
    .filter(([, i]) => i.status === "in_flight")
    .map(([r]) => r);
  const slots = Math.max(0, maxParallel - inFlight.length);
  const ready = Object.entries(issues)
    .filter(([, i]) => i.status === "ready")
    .map(([r]) => r);
  const epicCheckout = checkouts[`${owner}/${repo}`];
  const cloneRoot = dirname(epicCheckout ?? process.cwd());
  return {
    epic: {
      ref: `${owner}/${repo}#${number}`,
      title: epic.title,
      state: epic.state,
      url: epic.html_url,
    },
    state_dir: stateDir,
    max_parallel: maxParallel,
    issues: Object.values(issues),
    counts: Object.fromEntries(
      (["done", "in_flight", "ready", "blocked", "external_blocked"] as const).map((s) => [
        s,
        Object.values(issues).filter((i) => i.status === s).length,
      ]),
    ),
    in_flight: inFlight,
    ready,
    launch_batch: pickBatch(ready, issues, inFlight, slots),
    cycle,
    missing_checkouts: Object.entries(checkouts)
      .filter(([, p]) => p === null)
      .map(([r]) => r)
      .sort(),
    clone_root: cloneRoot,
    complete:
      Object.keys(issues).length > 0 && Object.values(issues).every((i) => i.state === "closed"),
  };
}

function renderTable(g: Record<string, unknown>): string {
  const epic = g.epic as { ref: string; state: string; title: string };
  const counts = g.counts as Record<string, number>;
  const issues = g.issues as GraphIssue[];
  const lines = [
    `Epic ${epic.ref} [${epic.state}] ${epic.title}`,
    "counts: " +
      Object.entries(counts)
        .map(([k, v]) => `${k}=${v}`)
        .join(", "),
    `${"issue".padEnd(34)} ${"status".padEnd(17)} ${"wave".padStart(4)} ${"chain".padStart(5)} ${"unblocks".padStart(8)}  open blockers`,
  ];
  const order: Record<string, number> = {
    in_flight: 0,
    ready: 1,
    blocked: 2,
    external_blocked: 3,
    done: 4,
  };
  const sorted = [...issues].sort((a, b) => {
    const oa = order[a.status ?? ""] ?? 99;
    const ob = order[b.status ?? ""] ?? 99;
    if (oa !== ob) return oa - ob;
    const wa = a.wave ?? 99;
    const wb = b.wave ?? 99;
    if (wa !== wb) return wa - wb;
    return a.ref < b.ref ? -1 : a.ref > b.ref ? 1 : 0;
  });
  for (const i of sorted) {
    const unblocks = i.status === "done" ? "-" : String(i.unblocks);
    const chain = i.status === "done" ? "-" : String(i.chain);
    const wave = i.wave == null ? "-" : String(i.wave);
    lines.push(
      `${i.ref.padEnd(34)} ${(i.status ?? "").padEnd(17)} ${wave.padStart(4)} ${chain.padStart(5)} ${unblocks.padStart(8)}  ${(i.open_blockers ?? []).join(", ") || "-"}`,
    );
  }
  const inFlight = g.in_flight as string[];
  const launchBatch = g.launch_batch as string[];
  lines.push(
    `launch batch (max ${String(g.max_parallel)}, ${inFlight.length} in flight): ${launchBatch.join(", ") || "none"}`,
  );
  const cycle = g.cycle as string[];
  if (cycle.length) lines.push("CYCLE (cannot schedule): " + cycle.join(", "));
  const missing = g.missing_checkouts as string[];
  if (missing.length) {
    lines.push(`no local checkout (clone into ${String(g.clone_root)}): ` + missing.join(", "));
  }
  if (g.complete) lines.push("ALL SUB-ISSUES CLOSED - epic complete");
  return lines.join("\n");
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let epic: string | undefined;
  let maxParallel = 3;
  let asJson = false;
  let outFile: string | undefined;
  let save = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--max") {
      const v = argv[++i];
      if (v === undefined) throw new Error("--max needs a value");
      maxParallel = Number(v);
    } else if (a === "--json") asJson = true;
    else if (a === "--out") {
      outFile = argv[++i];
      if (outFile === undefined) throw new Error("--out needs a path");
    } else if (a === "--save") save = true;
    else if (a !== undefined && !a.startsWith("-")) epic = a;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!epic) throw new Error("usage: epic-graph.ts EPIC [--max N] [--json] [--out FILE] [--save]");
  const graph = await build(epic, maxParallel);
  const targets: string[] = [];
  if (outFile) targets.push(outFile);
  if (save) targets.push(join(String(graph.state_dir), "graph.json"));
  for (const target of targets) {
    await mkdir(dirname(target), { recursive: true });
    await writeFile(target, `${JSON.stringify(graph, null, 2)}\n`);
  }
  process.stdout.write((asJson ? JSON.stringify(graph, null, 2) : renderTable(graph)) + "\n");
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
