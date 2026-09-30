#!/usr/bin/env bun
// @bun

// plugins/pr-babysit/hooks/src/babysit/common.ts
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync
} from "fs";
import { basename, dirname, join } from "path";

class BabysitError extends Error {
  code;
  extra;
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    this.extra = extra;
  }
}
function exitCode(code) {
  if (code === "usage")
    return 2;
  if (code === "duplicate_reply")
    return 4;
  if (code === "resolve_unconfirmed")
    return 5;
  if (code === "locked")
    return 75;
  return 3;
}
function errorDocument(error) {
  return { version: 1, errors: [{ code: error.code, message: error.message, ...error.extra }] };
}
function fail(code, message, extra = {}) {
  throw new BabysitError(code, message, extra);
}
function runCli(action) {
  Promise.resolve().then(action).then((result) => {
    if (result !== undefined)
      process.stdout.write(`${JSON.stringify(result)}
`);
  }).catch((error) => {
    if (error instanceof BabysitError) {
      process.stdout.write(`${JSON.stringify(errorDocument(error))}
`);
      process.exitCode = exitCode(error.code);
    } else {
      const message = error instanceof Error ? error.message : String(error);
      process.stdout.write(`${JSON.stringify(errorDocument(new BabysitError("api_error", message)))}
`);
      process.exitCode = 3;
    }
  });
}
function parseFlags(argv, command, valued, bare = []) {
  const flags = {};
  for (let i = 0;i < argv.length; i += 1) {
    const arg = argv[i] ?? "";
    if (bare.includes(arg)) {
      flags[arg] = true;
    } else if (valued.includes(arg)) {
      flags[arg] = argv[i + 1] ?? "";
      i += 1;
    } else {
      fail("usage", `${command}: unknown argument: ${arg}`);
    }
  }
  return flags;
}
function flag(flags, key) {
  const value = flags[key];
  return typeof value === "string" ? value : "";
}
function utcNow() {
  return new Date().toISOString().slice(0, 19) + "Z";
}
function atomicWriteJson(path, value, raw) {
  const content = raw ?? `${JSON.stringify(value)}
`;
  JSON.parse(content);
  mkdirSync(dirname(path), { recursive: true });
  const dir = mkdtempSync(join(dirname(path), `.${basename(path)}.tmp.${process.pid}.`));
  const temp = join(dir, "value");
  try {
    writeFileSync(temp, content);
    renameSync(temp, path);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// plugins/pr-babysit/hooks/src/babysit/gh.ts
class GhError extends Error {
  attempts;
  classification;
  lastMessage;
  lastRc;
  constructor(attempts, classification, lastMessage, lastRc) {
    super(lastMessage || `rc ${lastRc}`);
    this.attempts = attempts;
    this.classification = classification;
    this.lastMessage = lastMessage;
    this.lastRc = lastRc;
  }
}
function ghClassify(rc, stderr, stdout) {
  if (rc === 0)
    return "ok";
  if (rc === 124)
    return "transient";
  const combined = `${stderr}
${stdout}`;
  const fromStderr = /\(HTTP (\d{3})\)/.exec(combined)?.[1];
  let fromBody;
  try {
    const body = JSON.parse(stdout);
    if (body && typeof body === "object" && !Array.isArray(body)) {
      const status = body.status;
      if (typeof status === "string" || typeof status === "number")
        fromBody = String(status);
    }
  } catch {}
  const status = fromStderr ?? fromBody;
  if (status?.startsWith("5") || status === "429")
    return "transient";
  if (status === "403")
    return /rate limit/i.test(combined) ? "transient" : "permanent";
  if (status?.startsWith("4"))
    return "permanent";
  if (/connection refused|connection reset|no such host|i\/o timeout|TLS handshake|unexpected EOF|EOF$|network is unreachable|temporary failure|timed out/i.test(combined))
    return "transient";
  return "permanent";
}
async function ghRun(args, options = {}) {
  const timeoutSeconds = options.timeoutSeconds ?? Number(process.env.PB_GH_TIMEOUT ?? 60);
  const attempts = options.attempts ?? Number(process.env.PB_GH_ATTEMPTS ?? 3);
  const backoffSeconds = options.backoffSeconds ?? (process.env.PB_GH_BACKOFF ?? "2 4 8").split(/\s+/).map(Number);
  for (let attempt = 1;attempt <= attempts; attempt += 1) {
    const proc = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe", env: process.env });
    let timer;
    const timedOut = await Promise.race([
      proc.exited.then(() => false),
      new Promise((resolve) => {
        timer = setTimeout(() => {
          proc.kill();
          resolve(true);
        }, timeoutSeconds * 1000);
      })
    ]);
    if (timer)
      clearTimeout(timer);
    const [stdout, stderr] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text()
    ]);
    const rc = timedOut ? 124 : await proc.exited;
    const classification = ghClassify(rc, stderr, stdout);
    if (classification === "ok")
      return stdout;
    const lastMessage = stderr.split(/\r?\n/)[0] ?? "";
    if (classification === "permanent" || attempt === attempts)
      throw new GhError(attempt, classification, lastMessage, rc);
    const delay = backoffSeconds[attempt - 1] ?? 2;
    process.stderr.write(`pr-babysit: gh ${args[0]} attempt ${attempt} failed (${classification}: ${lastMessage || `rc ${rc}`}); retrying in ${delay}s
`);
    if (delay > 0)
      await Bun.sleep(delay * 1000);
  }
  throw new GhError(0, "permanent", "gh was not attempted", 1);
}
function ghJson(stdout) {
  const parsed = JSON.parse(stdout);
  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    const errors = parsed.errors;
    if (Array.isArray(errors) && errors.length > 0)
      throw new Error("response is not valid JSON or carries GraphQL errors[]");
  }
  return parsed;
}

// plugins/pr-babysit/hooks/src/babysit/verdict.ts
import { createHash } from "crypto";
var EMPTY = {
  is_review_comment: false,
  state: "unknown",
  complete: false,
  verdict: "none",
  verdict_label: "",
  findings: []
};
function parseVerdict(input) {
  if (!input.trim())
    return { ...EMPTY, findings: [] };
  const lines = input.split(/\n/);
  const reviewMarker = lines.some((line) => /^### Code Review|^### PR Review in Progress|\[View job\]\([^)]*actions\/runs\/[0-9]+|`agent-merge-[a-z]|^`(?:merge-approved|request-changes)`$/.test(line));
  if (!reviewMarker)
    return { ...EMPTY, findings: [] };
  const unchecked = lines.some((line) => /^\s*-\s*\[\s*\]/.test(line));
  const checked = lines.some((line) => /^\s*-\s*\[[xX]\]/.test(line));
  let state = !checked && !unchecked ? "unknown" : unchecked ? "in_progress" : "complete";
  let complete = state === "complete";
  let verdictLabel = input.match(/Set verdict label \(`([^`]+)`\)/)?.[1] ?? "";
  if (!verdictLabel) {
    verdictLabel = lines.flatMap((line) => {
      const match = line.match(/^`(agent-merge-[a-z-]+|merge-approved|request-changes)`$/);
      return match ? [match[1]] : [];
    }).at(-1) ?? "";
  }
  if (!verdictLabel)
    verdictLabel = input.match(/agent-merge-[a-z-]+/)?.[0] ?? "";
  let verdict;
  if (verdictLabel.includes("approved"))
    verdict = "approved";
  else if (verdictLabel.includes("blocked") || verdictLabel.includes("changes"))
    verdict = "changes";
  else {
    const summary = lines.find((line) => /^\*\*Verdict:\*\*/.test(line)) ?? "";
    if (/changes requested/i.test(summary))
      verdict = "changes";
    else if (/approved/i.test(summary))
      verdict = "approved";
    else if (/\*\*Changes requested\*\*|changes-requested/i.test(input))
      verdict = "changes";
    else if (/\*\*Approved\*\*/i.test(input))
      verdict = "approved";
    else
      verdict = "none";
  }
  const verdictLine = lines.find((line) => /^\*\*Verdict:\*\*/.test(line)) ?? "";
  if (/review incomplete|provider error/i.test(verdictLine)) {
    state = "provider_error";
    complete = false;
  }
  const findings = [];
  let inFindings = false;
  for (const line of lines) {
    if (/^### Findings(?:\s|$)/.test(line)) {
      inFindings = true;
      continue;
    }
    if (/^### /.test(line))
      inFindings = false;
    if (!inFindings)
      continue;
    const match = line.match(/^`([^`]+)`: (blocker|high|medium|low|nit): (.*)$/);
    if (!match)
      continue;
    const [, rawPath, severity, text] = match;
    const pathLine = rawPath.match(/^(.+):([0-9]+)$/);
    const path = pathLine?.[1] ?? rawPath;
    const lineNumber = pathLine ? Number(pathLine[2]) : null;
    const hash = createHash("sha1").update(text).digest("hex").slice(0, 8);
    findings.push({
      path,
      line: lineNumber,
      severity,
      text,
      key: `${path}:${pathLine?.[2] ?? ""}:${hash}`
    });
  }
  const mustFix = [];
  let inTopN = false;
  for (const line of lines) {
    if (/^###\s+Top-N\s+must-fix(?:\s|$)/i.test(line)) {
      inTopN = true;
      continue;
    }
    if (/^### |^<details>/.test(line))
      inTopN = false;
    if (!inTopN)
      continue;
    let item = line.trim();
    if (!item)
      continue;
    item = item.replace(/^[-*] /, "").replace(/^[0-9]+\.\s+/, "");
    if (item)
      mustFix.push(item);
  }
  return {
    is_review_comment: true,
    state,
    complete,
    verdict,
    verdict_label: verdictLabel,
    findings,
    must_fix: mustFix
  };
}

// plugins/pr-babysit/hooks/src/babysit/collect.ts
var PR_FIELDS = "number,title,url,author,state,baseRefName,headRefName,headRefOid,statusCheckRollup,mergeable,reviewDecision";
var THREADS_QUERY = `query($owner:String!,$repo:String!,$number:Int!,$pageSize:Int!,$endCursor:String){
  repository(owner:$owner,name:$repo){ pullRequest(number:$number){
    reviewThreads(first:$pageSize,after:$endCursor){
      pageInfo{hasNextPage endCursor}
      nodes{ id isResolved isOutdated path line
        comments(first:$pageSize){ pageInfo{hasNextPage endCursor}
          nodes{ id databaseId body author{login __typename} createdAt url } } } } } } }`;
var COMMENTS_QUERY = `query($id:ID!,$pageSize:Int!,$endCursor:String){
  node(id:$id){ ... on PullRequestReviewThread {
    comments(first:$pageSize,after:$endCursor){ pageInfo{hasNextPage endCursor}
      nodes{ id databaseId body author{login __typename} createdAt url } } } } }`;
var obj = (value) => value && typeof value === "object" ? value : {};
var arr = (value) => Array.isArray(value) ? value : [];
var n = (value) => value ?? null;
function threadComment(raw) {
  return {
    id: n(raw.id),
    databaseId: n(raw.databaseId),
    body: n(raw.body),
    author: n(obj(raw.author).login),
    authorType: n(obj(raw.author).__typename),
    createdAt: n(raw.createdAt),
    url: n(raw.url)
  };
}
function normalizeThreads(pages) {
  return arr(pages).flatMap((page) => arr(obj(obj(obj(page).data).repository).pullRequest?.reviewThreads?.nodes)).map((raw) => ({
    id: n(raw.id),
    isResolved: n(raw.isResolved),
    isOutdated: n(raw.isOutdated),
    path: n(raw.path),
    line: n(raw.line),
    comments: arr(obj(raw.comments).nodes).map(threadComment),
    commentsHasNextPage: n(obj(obj(raw.comments).pageInfo).hasNextPage),
    commentsEndCursor: n(obj(obj(raw.comments).pageInfo).endCursor)
  }));
}
function normalizeThreadComments(pages) {
  return arr(pages).flatMap((page) => arr(obj(obj(page).data).node?.comments?.nodes)).map(threadComment);
}
function normalizeComments(pages) {
  return arr(pages).flatMap((page) => arr(page)).map((raw) => ({
    id: n(raw.id),
    body: n(raw.body),
    author: n(obj(raw.user).login),
    authorType: n(obj(raw.user).type),
    createdAt: n(raw.created_at),
    updatedAt: n(raw.updated_at),
    url: n(raw.html_url)
  }));
}
function normalizeReviews(pages) {
  return arr(pages).flatMap((page) => arr(page)).map((raw) => ({
    id: n(raw.id),
    state: n(raw.state),
    body: n(raw.body),
    author: n(obj(raw.user).login),
    authorType: n(obj(raw.user).type),
    submittedAt: n(raw.submitted_at),
    url: n(raw.html_url),
    commitId: n(raw.commit_id)
  }));
}
var CI_REVIEWERS = new Set(["github-actions", "github-actions[bot]", "claude", "claude[bot]"]);
function isCiReviewer(login) {
  return typeof login === "string" && CI_REVIEWERS.has(login);
}
function botComment(comments) {
  const candidates = comments.filter((comment) => isCiReviewer(comment.author)).reverse();
  let chosen;
  let verdict;
  for (const comment of candidates) {
    const parsed = parseVerdict(String(comment.body ?? ""));
    if (parsed.is_review_comment === true) {
      chosen = comment;
      verdict = parsed;
      break;
    }
  }
  if (!chosen) {
    chosen = candidates[0];
    verdict = chosen ? parseVerdict(String(chosen.body ?? "")) : { ...parseVerdict(""), state: "absent" };
  }
  return {
    comment: chosen ? {
      id: chosen.id,
      url: chosen.url,
      createdAt: chosen.createdAt,
      updatedAt: chosen.updatedAt,
      author: chosen.author
    } : null,
    verdict
  };
}
function readError(source, error, prefix = "collect-pr.sh: read") {
  if (error instanceof BabysitError)
    throw error;
  if (error instanceof GhError) {
    throw new BabysitError("api_error", `${prefix} '${source}' failed: ${error.lastMessage}`, {
      source,
      attempts: error.attempts,
      class: error.classification,
      lastMessage: error.lastMessage
    });
  }
  throw new BabysitError("invalid_json", `${prefix} '${source}' failed: response is not valid JSON or carries GraphQL errors[]`, {
    source,
    attempts: 1,
    class: "invalid_json",
    lastMessage: "response is not valid JSON or carries GraphQL errors[]"
  });
}
async function collectPr(options) {
  const { repo, pr, pageSize = 100, timeoutSeconds, gh = ghRun, now = utcNow } = options;
  const [owner, name] = repo.split("/");
  const transport = (args) => gh(args, timeoutSeconds === undefined ? {} : { timeoutSeconds });
  const read = async (source, args) => {
    try {
      return ghJson(await transport(args));
    } catch (error) {
      return readError(source, error);
    }
  };
  const head = async () => {
    let doc;
    try {
      doc = obj(ghJson(await transport(["pr", "view", String(pr), "--repo", repo, "--json", "headRefOid"])));
    } catch (error) {
      if (error instanceof GhError)
        throw new BabysitError("api_error", `gh head failed after ${error.attempts} attempt(s): ${error.lastMessage || `rc ${error.lastRc}`}`, {
          source: "head",
          attempts: error.attempts,
          class: error.classification,
          lastMessage: error.lastMessage
        });
      throw new BabysitError("invalid_json", "collect-pr.sh: head read returned no headRefOid", {
        source: "head"
      });
    }
    if (typeof doc.headRefOid !== "string" || doc.headRefOid === "")
      throw new BabysitError("invalid_json", "collect-pr.sh: head read returned no headRefOid", {
        source: "head"
      });
    return doc.headRefOid;
  };
  let recollected = false;
  for (let round = 0;round < 2; round += 1) {
    const before = await head();
    const reads = [
      ["pr", ["pr", "view", String(pr), "--repo", repo, "--json", PR_FIELDS]],
      [
        "threads",
        [
          "api",
          "graphql",
          "--paginate",
          "--slurp",
          "-f",
          `owner=${owner}`,
          "-f",
          `repo=${name}`,
          "-F",
          `number=${pr}`,
          "-F",
          `pageSize=${pageSize}`,
          "-f",
          `query=${THREADS_QUERY}`
        ]
      ],
      [
        "comments",
        [
          "api",
          "--paginate",
          "--slurp",
          `repos/${repo}/issues/${pr}/comments?per_page=${pageSize}`
        ]
      ],
      [
        "reviews",
        ["api", "--paginate", "--slurp", `repos/${repo}/pulls/${pr}/reviews?per_page=${pageSize}`]
      ]
    ];
    const settled = await Promise.allSettled(reads.map(([source, args]) => read(source, args)));
    for (let i = 0;i < settled.length; i += 1) {
      const outcome = settled[i];
      if (outcome.status === "rejected")
        throw outcome.reason;
    }
    const [prRaw, threadPages, commentPages, reviewPages] = settled.map((outcome) => outcome.value);
    const threads = normalizeThreads(threadPages);
    const overflowingThreads = threads.filter((thread) => thread.commentsHasNextPage === true);
    const commentReads = await Promise.allSettled(overflowingThreads.map((thread) => read("threadComments", [
      "api",
      "graphql",
      "--paginate",
      "--slurp",
      "-f",
      `id=${thread.id}`,
      "-F",
      `pageSize=${pageSize}`,
      "-f",
      `endCursor=${thread.commentsEndCursor}`,
      "-f",
      `query=${COMMENTS_QUERY}`
    ])));
    for (const outcome of commentReads) {
      if (outcome.status === "rejected")
        throw outcome.reason;
    }
    let threadCommentPages = 0;
    for (let i = 0;i < overflowingThreads.length; i += 1) {
      const thread = overflowingThreads[i];
      const pages = commentReads[i].value;
      threadCommentPages += arr(pages).length;
      thread.comments.push(...normalizeThreadComments(pages));
      thread.commentsHasNextPage = false;
      thread.commentsEndCursor = null;
    }
    const comments = normalizeComments(commentPages);
    const reviews = normalizeReviews(reviewPages);
    const bot = botComment(comments);
    const after = await head();
    if (before !== after) {
      recollected = true;
      continue;
    }
    const raw = obj(prRaw);
    return {
      version: 1,
      collectedAt: now(),
      repo,
      number: pr,
      head: { sha: after, verifiedAfterFanout: true, recollected },
      pageSize,
      pr: {
        number: n(raw.number),
        title: n(raw.title),
        url: n(raw.url),
        author: n(obj(raw.author).login),
        state: n(raw.state),
        baseRefName: n(raw.baseRefName),
        headRefName: n(raw.headRefName),
        headRefOid: n(raw.headRefOid),
        mergeable: n(raw.mergeable),
        reviewDecision: n(raw.reviewDecision),
        statusCheckRollup: raw.statusCheckRollup ?? []
      },
      threads: threads.map(({ commentsHasNextPage: _hasNext, commentsEndCursor: _cursor, ...thread }) => thread),
      comments,
      reviews,
      bot,
      pages: {
        threads: arr(threadPages).length,
        threadComments: threadCommentPages,
        comments: arr(commentPages).length,
        reviews: arr(reviewPages).length
      }
    };
  }
  throw new BabysitError("head_moved", "collect-pr.sh: PR head moved twice during collection", {
    source: "head"
  });
}

// plugins/pr-babysit/hooks/src/babysit-collect-pr.ts
runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "collect-pr.sh", [
    "--repo",
    "--pr",
    "--out",
    "--page-size",
    "--timeout"
  ]);
  const repo = flag(flags, "--repo");
  const prText = flag(flags, "--pr");
  const out = flag(flags, "--out");
  const pageText = flag(flags, "--page-size") || "100";
  const timeoutText = flag(flags, "--timeout") || process.env.PB_GH_TIMEOUT || "60";
  if (!/^[^/\s]+\/[^/\s]+$/.test(repo))
    fail("usage", "collect-pr.sh: --repo <owner/repo> required");
  if (!/^\d+$/.test(prText))
    fail("usage", "collect-pr.sh: --pr <n> required");
  if (!out)
    fail("usage", "collect-pr.sh: --out <path> required");
  if (!/^\d+$/.test(pageText) || Number(pageText) < 1)
    fail("usage", "collect-pr.sh: --page-size must be a positive integer");
  if (!/^\d+$/.test(timeoutText) || Number(timeoutText) < 1)
    fail("usage", "collect-pr.sh: --timeout must be a positive integer");
  if (!Bun.which("gh"))
    fail("gh_unavailable", "gh is required");
  const snapshot = await collectPr({
    repo,
    pr: Number(prText),
    pageSize: Number(pageText),
    timeoutSeconds: Number(timeoutText)
  });
  try {
    atomicWriteJson(out, snapshot);
  } catch {
    fail("invalid_json", "collect-pr.sh: could not assemble the snapshot", { source: "snapshot" });
  }
  process.stdout.write(`${out}
`);
});
