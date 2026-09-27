/** GitHub epics: native sub-issues (or a task-list fallback), dependency API
 * plus "blocked by" text, closing-PR references, and checklist ticks. */

import { CommandError, ghJson, parseRef, run } from "../common.ts";
import { mapLimit } from "../ratelimit.ts";
import { excerpt, type EpicInfo, type PrNode, type TrackedIssue, type Tracker } from "./types.ts";

const DEP_LINE = /\b(?:blocked by|depends on)\b[^\n]*/gi;
/** URL, owner/repo#N, or bare #N (default repo supplied by parseRef). */
export const ISSUE_REF =
  /https:\/\/github\.com\/[^\s)]+\/issues\/\d+|[\w.-]+\/[\w.-]+#\d+|(?<![\w/])#\d+/g;
const CLOSING_PRS = `query($o:String!,$r:String!,$n:Int!){repository(owner:$o,name:$r){issue(number:$n){
closedByPullRequestsReferences(first:10,includeClosedPrs:true){nodes{number state url headRefName}}}}}`;
/** Concurrent sub-issue fetches: enough to be quick, low enough to stay under
 * GitHub's secondary (burst) rate limit. */
const FETCH_CONCURRENCY = 4;

type IssueItem = {
  repository_url: string;
  number: number;
  title: string;
  state: string;
  html_url: string;
  body?: string | null;
  labels?: ({ name?: string } | string)[];
};

type SubIssue = { ref: string; title: string; state: string; url: string };

function refOf(item: IssueItem): string {
  const parts = item.repository_url.replace(/\/$/, "").split("/");
  const owner = parts[parts.length - 2];
  const repo = parts[parts.length - 1];
  if (!owner || !repo) throw new Error(`bad repository_url: ${item.repository_url}`);
  return `${owner}/${repo}#${item.number}`;
}

function refsIn(text: string, defaultRepo: string): string[] {
  const out: string[] = [];
  const re = new RegExp(ISSUE_REF.source, ISSUE_REF.flags);
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const [o, r, n] = parseRef(m[0], defaultRepo);
    out.push(`${o}/${r}#${n}`);
  }
  return out;
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
    return subs.map((i) => ({ ref: refOf(i), title: i.title, state: i.state, url: i.html_url }));
  }
  const refs: string[] = [];
  for (const line of body.split("\n")) {
    if (/^\s*[-*]\s*\[[ xX]\]/.test(line)) refs.push(...refsIn(line, `${owner}/${repo}`));
  }
  return mapLimit([...new Set(refs)], FETCH_CONCURRENCY, async (ref) => {
    const [o, r, n] = parseRef(ref);
    const i = (await ghJson(["api", `repos/${o}/${r}/issues/${n}`])) as IssueItem;
    return { ref, title: i.title, state: i.state, url: i.html_url };
  });
}

function labelNames(labels: IssueItem["labels"]): string[] {
  return (labels ?? []).map((l) => (typeof l === "string" ? l : (l.name ?? ""))).filter(Boolean);
}

async function fetchDetails(sub: SubIssue): Promise<TrackedIssue> {
  const [owner, repo, number] = parseRef(sub.ref);
  const blockers: Record<string, string> = {};
  let depsApi = true;
  try {
    const blocked =
      ((await ghJson([
        "api",
        `repos/${owner}/${repo}/issues/${number}/dependencies/blocked_by`,
      ])) as IssueItem[] | null) ?? [];
    for (const b of blocked) blockers[refOf(b)] = b.state;
  } catch (err) {
    if (err instanceof CommandError) depsApi = false;
    else throw err;
  }
  const item = (await ghJson([
    "api",
    `repos/${owner}/${repo}/issues/${number}`,
    "--jq",
    "{body: .body, labels: .labels}",
  ])) as { body: string | null; labels: IssueItem["labels"] };
  const body = item.body ?? "";
  for (const line of body.match(DEP_LINE) ?? []) {
    for (const ref of refsIn(line, `${owner}/${repo}`)) {
      if (ref in blockers || ref === sub.ref) continue;
      const [o, r, n] = parseRef(ref);
      const st = (await ghJson(["api", `repos/${o}/${r}/issues/${n}`, "--jq", "{s: .state}"])) as {
        s: string;
      };
      blockers[ref] = st.s;
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
    ref: sub.ref,
    title: sub.title,
    state: sub.state === "closed" ? "closed" : "open",
    url: sub.url,
    repo: `${owner}/${repo}`,
    number,
    blockers,
    prs,
    deps_source: depsApi ? "native+text" : "text",
    labels: labelNames(item.labels),
    excerpt: excerpt(body),
  };
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

export class GitHubTracker implements Tracker {
  readonly kind = "github";
  readonly epicRef: string;
  private readonly owner: string;
  private readonly repo: string;
  private readonly number: number;
  private body = "";

  constructor(ref: string) {
    [this.owner, this.repo, this.number] = parseRef(ref);
    this.epicRef = `${this.owner}/${this.repo}#${this.number}`;
  }

  async epic(): Promise<EpicInfo> {
    const e = (await ghJson(["api", `repos/${this.owner}/${this.repo}/issues/${this.number}`])) as {
      title: string;
      state: string;
      html_url: string;
      body?: string | null;
    };
    this.body = e.body ?? "";
    return {
      ref: this.epicRef,
      title: e.title,
      state: e.state === "closed" ? "closed" : "open",
      url: e.html_url,
    };
  }

  async children(): Promise<TrackedIssue[]> {
    const subs = await fetchSubIssues(this.owner, this.repo, this.number, this.body);
    return mapLimit(subs, FETCH_CONCURRENCY, fetchDetails);
  }

  async closeIssue(issue: string, note: string): Promise<string> {
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
    await run(
      [
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
      ],
      { write: true },
    );
    return "closed-manually";
  }

  async tickEpic(issue: string, title: string): Promise<boolean> {
    const bodyResp = (await ghJson([
      "api",
      `repos/${this.owner}/${this.repo}/issues/${this.number}`,
      "--jq",
      "{b: .body}",
    ])) as { b: string | null };
    const body = bodyResp.b ?? "";
    const next = tickBody(body, [this.owner, this.repo], issue, title);
    if (next === body) return false;
    await run(
      [
        "gh",
        "api",
        "-X",
        "PATCH",
        `repos/${this.owner}/${this.repo}/issues/${this.number}`,
        "-f",
        `body=${next}`,
      ],
      { write: true },
    );
    return true;
  }

  async closeEpic(summary: string): Promise<string> {
    const target = [String(this.number), "-R", `${this.owner}/${this.repo}`];
    await run(["gh", "issue", "comment", ...target, "--body", summary], { write: true });
    await run(["gh", "issue", "close", ...target, "--reason", "completed"], { write: true });
    return "closed";
  }

  stateSlug(): string {
    return `${this.owner}-${this.repo}-${this.number}`.toLowerCase();
  }
}
