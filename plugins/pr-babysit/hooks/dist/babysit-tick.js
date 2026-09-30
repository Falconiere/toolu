#!/usr/bin/env bun
// @bun

// plugins/pr-babysit/hooks/src/babysit-tick.ts
import { existsSync as existsSync2, readFileSync as readFileSync2 } from "fs";

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
function readTextOrEmpty(path) {
  try {
    return readFileSync(path, "utf8").trim();
  } catch {
    return "";
  }
}
function pidAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

class SlotLock {
  path;
  held = false;
  onTerm = () => {
    this.release();
    process.exit(143);
  };
  onInt = () => {
    this.release();
    process.exit(130);
  };
  constructor(statePath) {
    this.path = `${statePath}.lock`;
  }
  acquire() {
    mkdirSync(dirname(this.path), { recursive: true });
    for (let attempt = 0;attempt < 2; attempt += 1) {
      try {
        mkdirSync(this.path);
        this.held = true;
        writeFileSync(join(this.path, "pid"), `${process.pid}
`);
        writeFileSync(join(this.path, "since"), `${Math.floor(Date.now() / 1000)}
`);
        process.on("SIGTERM", this.onTerm);
        process.on("SIGINT", this.onInt);
        return;
      } catch (error) {
        if (!existsSync(this.path))
          throw error;
      }
      const pidText = readTextOrEmpty(join(this.path, "pid"));
      const sinceText = readTextOrEmpty(join(this.path, "since"));
      const pid = /^\d+$/.test(pidText) ? Number(pidText) : null;
      const since = /^\d+$/.test(sinceText) ? Number(sinceText) : null;
      const staleAfter = Number(process.env.PB_LOCK_STALE_SECONDS ?? "600");
      const stale = pid === null || since === null || !pidAlive(pid) || Math.floor(Date.now() / 1000) - since > staleAfter;
      if (attempt === 0 && stale) {
        process.stderr.write(`pr-babysit: reclaiming stale lock ${this.path} (pid ${pidText || "?"}, since ${sinceText || "?"})
`);
        rmSync(this.path, { recursive: true, force: true });
        continue;
      }
      fail("locked", "slot is held by another controller", { pid, since });
    }
  }
  release() {
    process.off("SIGTERM", this.onTerm);
    process.off("SIGINT", this.onInt);
    if (this.held && readTextOrEmpty(join(this.path, "pid")) === String(process.pid)) {
      rmSync(this.path, { recursive: true, force: true });
    }
    this.held = false;
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
    verdict = parseVerdict(String(chosen?.body ?? ""));
    if (!chosen)
      verdict.state = "absent";
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
    let threadCommentPages = 0;
    for (const thread of threads) {
      if (thread.commentsHasNextPage !== true)
        continue;
      const pages = await read("threadComments", [
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
      ]);
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

// plugins/pr-babysit/hooks/src/babysit/reduce.ts
var ciReviewers = new Set(["github-actions", "github-actions[bot]", "claude", "claude[bot]"]);
var injectionPatterns = [
  "ignore (all |any |the )?(previous|prior|above|earlier) (instructions|prompts?|rules)",
  "disregard (all |any |the |your )?(previous|prior|system|above) ",
  "you are (now )?(an? )?(ai|assistant|llm|language model|claude|codex|copilot)",
  "(^|\\n)\\s*(system|assistant)\\s*:",
  "<(system|instructions?)>",
  "(run|execute) (the following|this|these) (command|shell|script)"
].map((pattern) => new RegExp(pattern, "i"));
var nil = (value, fallback) => value === null || value === undefined || value === false ? fallback : value;
var asArray = (value) => Array.isArray(value) ? value : [];
var jqString = (value) => typeof value === "string" ? value : JSON.stringify(value);
var reason = (code, detail) => ({ code, detail });
function checkState(check) {
  if (check.__typename === "StatusContext") {
    return check.state === "SUCCESS" ? "pass" : check.state === "PENDING" || check.state === "EXPECTED" ? "pending" : "fail";
  }
  return check.status !== "COMPLETED" ? "pending" : ["SUCCESS", "NEUTRAL", "SKIPPED"].includes(check.conclusion) ? "pass" : "fail";
}
function ciStatus(checks) {
  if (checks.length === 0)
    return "pending";
  const states = checks.map((check) => check.status);
  return states.includes("fail") ? "fail" : states.includes("pending") ? "pending" : "pass";
}
function classifiedThread(thread, author, actions) {
  const comments = asArray(thread.comments);
  const lastComment = comments.at(-1) ?? null;
  const lastNonAuthor = comments.filter((comment) => comment.author !== author).at(-1) ?? null;
  const flagged = Object.keys(actions.flagged).includes(thread.id);
  const authorClass = lastNonAuthor === null ? "none" : ciReviewers.has(lastNonAuthor.author) ? "ci_reviewer" : lastNonAuthor.authorType === "Bot" ? "bot" : "human";
  const open = thread.isResolved === false;
  const answerable = open && !flagged && lastComment !== null && lastComment.author !== author && (authorClass === "human" || authorClass === "ci_reviewer");
  const actionable = thread.isOutdated ? answerable && authorClass === "human" : answerable;
  const audited = open && !thread.isOutdated && !flagged;
  const injectionPattern = injectionPatterns.find((pattern) => pattern.test(nil(lastNonAuthor?.body, "")))?.source ?? null;
  return {
    id: thread.id,
    path: thread.path,
    line: thread.line,
    isOutdated: thread.isOutdated,
    isResolved: thread.isResolved,
    rootCommentId: nil(comments[0]?.databaseId, null),
    inReplyTo: nil(lastNonAuthor?.databaseId, null),
    authorClass,
    lastCommentAuthor: nil(lastComment?.author, null),
    lastCommentAt: nil(lastComment?.createdAt, null),
    injectionSuspect: injectionPattern !== null,
    injectionPattern,
    comments,
    flags: {
      actionable,
      audited,
      flagged,
      skippedOutdated: open && thread.isOutdated && authorClass === "ci_reviewer",
      replied: Object.keys(actions.replied).includes(`thread:${thread.id}@${jqString(nil(lastNonAuthor?.databaseId, 0))}`)
    }
  };
}
function reduceState(snapshot, previous, now, statePath, snapshotPath) {
  const snap = snapshot;
  const prev = previous;
  const pr = snap.pr;
  const head = snap.head.sha;
  const author = pr.author;
  const slot = `${snap.repo.toLowerCase().replaceAll("/", "-")}-${snap.number}`;
  const key = `${snap.repo}#${snap.number}`;
  const actions = prev === null ? { replied: {}, resolved: {}, flagged: {} } : nil(prev.actions, { replied: {}, resolved: {}, flagged: {} });
  const rollup = asArray(pr.statusCheckRollup);
  const checks = rollup.map((check) => ({
    name: nil(check.name, nil(check.context, "unknown")),
    status: checkState(check),
    url: nil(check.detailsUrl, nil(check.targetUrl, null))
  }));
  const ci = ciStatus(checks);
  const verdict = nil(snap.bot?.verdict, {});
  const botComment = snap.bot?.comment ?? null;
  const botState = nil(verdict.state, "absent");
  const botVerdict = nil(verdict.verdict, "none");
  const findings = asArray(verdict.findings);
  const keys = findings.map((finding) => finding.key);
  const findingsCount = findings.length;
  const degraded = botState === "absent" || botState === "unknown" || verdict.is_review_comment === false;
  const degradedReason = botState === "absent" ? "review_absent" : degraded ? "review_unknown_format" : null;
  const sameRun = prev !== null && botComment !== null && nil(prev.pr?.botCommentId, null) === botComment.id && nil(prev.pr?.botCommentUpdatedAt, null) === botComment.updatedAt;
  const threads = asArray(snap.threads).map((thread) => classifiedThread(thread, author, actions));
  const fixer = prev === null ? null : nil(prev.fixer, null);
  const fixerActive = fixer !== null && (fixer.status === "running" || fixer.status === "blocked");
  const fixingIds = fixerActive ? asArray(fixer.items).map(jqString) : [];
  const owned = (item) => fixingIds.includes(jqString(item.id));
  const allActionable = threads.filter((thread) => thread.flags.actionable).map(({ flags: _flags, isResolved: _resolved, ...rest }) => rest);
  const actionable = allActionable.filter((item) => !owned(item));
  const fixing = allActionable.filter(owned);
  const audited = threads.filter((thread) => thread.flags.audited);
  const staleUnresolved = audited.filter((thread) => !thread.flags.actionable).map((thread) => ({
    id: thread.id,
    path: thread.path,
    line: thread.line,
    repliedAt: thread.lastCommentAt,
    lastCommentAuthor: thread.lastCommentAuthor
  }));
  const skippedOutdated = threads.filter((thread) => thread.flags.skippedOutdated).map((thread) => thread.id);
  const flaggedInjection = threads.filter((thread) => thread.flags.flagged).map((thread) => thread.id);
  const unresolved = audited.length;
  const comments = asArray(snap.comments);
  const repliedKeys = Object.keys(actions.replied);
  const allConvActionable = comments.filter((comment) => comment.author !== author && comment.authorType !== "Bot").filter((comment) => !comments.some((later) => later.author === author && later.createdAt > comment.createdAt)).filter((comment) => !repliedKeys.includes(`conversation:${jqString(comment.id)}`)).map(({ id, author, body, createdAt, url }) => ({ id, author, body, createdAt, url }));
  const convActionable = allConvActionable.filter((item) => !owned(item));
  const convFixing = allConvActionable.filter(owned);
  const allReviewActionable = asArray(snap.reviews).filter((review) => review.author !== author && review.authorType !== "Bot" && review.state !== "APPROVED" && nil(review.body, "").length > 0).filter((review) => !repliedKeys.includes(`review:${jqString(review.id)}`)).map(({ id, author, state, body, submittedAt, url }) => ({
    id,
    author,
    state,
    body,
    submittedAt,
    url
  }));
  const reviewActionable = allReviewActionable.filter((item) => !owned(item));
  const reviewFixing = allReviewActionable.filter(owned);
  const lastRoundKeys = prev === null ? [] : asArray(prev.pr?.lastRoundFindingKeys);
  const lastRoundHadRejection = prev === null ? false : nil(prev.pr?.lastRoundHadRejection, false);
  const prevStreak = prev === null ? 0 : nil(prev.pr?.recurrenceStreak, 0);
  const fixAttempts = prev === null ? 0 : nil(prev.pr?.fixAttempts, 0);
  const recurringKeys = sameRun || prev === null ? [] : keys.filter((findingKey) => lastRoundKeys.includes(findingKey));
  const streak = sameRun ? prevStreak : recurringKeys.length > 0 ? prevStreak + 1 : 0;
  const comparison = {
    ciStatus: ci,
    reviewDecision: pr.reviewDecision,
    mergeable: pr.mergeable,
    unresolvedThreads: unresolved,
    headSha: head,
    botVerdict,
    botState,
    botFindingKeys: keys
  };
  const previousComparison = prev?.pr && {
    ciStatus: prev.pr.ciStatus,
    reviewDecision: prev.pr.reviewDecision,
    mergeable: prev.pr.mergeable,
    unresolvedThreads: prev.pr.unresolvedThreads,
    headSha: prev.pr.headSha,
    botVerdict: prev.pr.botVerdict,
    botState: prev.pr.botState,
    botFindingKeys: prev.pr.botFindingKeys
  };
  const changed = prev === null || JSON.stringify(previousComparison) !== JSON.stringify(comparison);
  const idleStreak = changed || fixerActive && fixer.status === "running" ? 0 : nil(prev?.idleStreak, 0) + 1;
  const intervalMinutes = idleStreak >= 9 ? 15 : idleStreak >= 6 ? 12 : idleStreak >= 3 ? 6 : ci === "fail" ? 1 : 3;
  const waitSeconds = idleStreak >= 6 ? 60 : idleStreak >= 3 ? 30 : 15;
  const providerErrorRepeated = prev !== null && botState === "provider_error" && nil(prev.pr?.botState, "") === "provider_error" && nil(prev.pr?.headSha, "") === head;
  const escalations = [];
  if (pr.state === "MERGED")
    escalations.push(reason("pr_merged", "PR is merged"));
  if (pr.state === "CLOSED")
    escalations.push(reason("pr_closed", "PR is closed"));
  if (pr.mergeable === "CONFLICTING")
    escalations.push(reason("merge_conflict", "mergeable is CONFLICTING"));
  if (fixAttempts >= 5)
    escalations.push(reason("fix_attempts_exhausted", `${fixAttempts} fix attempts recorded`));
  if (recurringKeys.length > 0 && lastRoundHadRejection)
    escalations.push(reason("recurrence_after_rejection", `${recurringKeys.length} finding key(s) recurred after a Won't-fix round`));
  if (recurringKeys.length > 0 && streak >= 2)
    escalations.push(reason("recurrence_streak", `finding keys recurred on ${streak} consecutive rounds`));
  if (providerErrorRepeated)
    escalations.push(reason("provider_error_repeated", `review provider error twice on head ${head.slice(0, 8)}`));
  const signals = [];
  if (ci === "pass")
    signals.push(reason("ci_pass", `${checks.length} check(s) passed`));
  else if (ci === "fail")
    signals.push(reason("ci_failed", checks.filter((check) => check.status === "fail").map((check) => check.name).join(", ")));
  else
    signals.push(reason("ci_pending", checks.length === 0 ? "no checks reported yet" : checks.filter((check) => check.status === "pending").map((check) => check.name).join(", ")));
  if (unresolved === 0)
    signals.push(reason("threads_clear", "no unresolved review threads"));
  if (actionable.length > 0)
    signals.push(reason("threads_unresolved", `${actionable.length} actionable thread(s)`));
  if (staleUnresolved.length > 0)
    signals.push(reason("threads_stale_unresolved", `${staleUnresolved.length} replied-but-unresolved thread(s)`));
  if (botState === "in_progress")
    signals.push(reason("review_in_progress", "review bot still running"));
  else if (botState === "provider_error" && !providerErrorRepeated)
    signals.push(reason("provider_error", "review bot reported a provider error; rerun the review job once"));
  else if (botState === "complete" && botVerdict === "approved" && findingsCount === 0)
    signals.push(reason("review_approved", "bot verdict approved with zero findings"));
  else if (botState === "complete")
    signals.push(reason("review_changes", `bot verdict ${botVerdict} with ${findingsCount} finding(s)`));
  else if (degraded)
    signals.push(reason(degradedReason, "bot verdict cannot be read"));
  if (degraded)
    signals.push(reason("manual_verify", `verify review findings manually: ${nil(botComment?.url, "no bot comment")}`));
  if (pr.mergeable === "UNKNOWN" && pr.state === "OPEN")
    signals.push(reason("mergeable_unknown", "GitHub has not computed mergeability yet"));
  if (fixerActive)
    signals.push(reason("fixer_running", `fixer ${fixer.status}: group ${nil(fixer.current, 1)} of ${asArray(fixer.groups).length}; ${fixingIds.length} item(s) in flight`));
  if (!changed)
    signals.push(reason("unchanged", "nothing changed since the last tick"));
  const successReady = pr.state === "OPEN" && ci === "pass" && unresolved === 0 && pr.mergeable !== "UNKNOWN" && !fixerActive && (botState === "complete" && botVerdict === "approved" && findingsCount === 0 || degraded);
  const decision = escalations.length > 0 ? "escalate" : successReady ? "success" : "keep_going";
  return {
    state: {
      version: 2,
      slot,
      repo: snap.repo,
      number: snap.number,
      cronName: `pr-babysit:${slot}`,
      lastUpdate: now,
      totalTicks: nil(prev?.totalTicks, 0) + 1,
      idleStreak,
      currentInterval: intervalMinutes,
      waitSeconds,
      status: nil(prev?.status, "active"),
      worktree: nil(prev?.worktree, null),
      fixer,
      herdrWorktree: nil(prev?.herdrWorktree, null),
      hostCooldowns: nil(prev?.hostCooldowns, {}),
      pr: {
        key,
        ciStatus: ci,
        reviewDecision: pr.reviewDecision,
        mergeable: pr.mergeable,
        unresolvedThreads: unresolved,
        headSha: head,
        fixAttempts,
        botVerdict,
        botState,
        botCommentId: nil(botComment?.id, null),
        botCommentUpdatedAt: nil(botComment?.updatedAt, null),
        botFindingKeys: keys,
        lastRoundFindingKeys: lastRoundKeys,
        lastRoundHadRejection,
        recurrenceStreak: streak,
        unresolvedAfterClearance: unresolved,
        lastError: null
      },
      actions,
      lastGoodSnapshot: snapshotPath
    },
    result: {
      version: 1,
      slot,
      changed,
      decision,
      reasons: [...escalations, ...signals],
      pr: {
        number: pr.number,
        url: pr.url,
        head,
        branch: pr.headRefName,
        base: pr.baseRefName,
        author,
        state: pr.state,
        mergeable: pr.mergeable,
        reviewDecision: pr.reviewDecision
      },
      ci: { status: ci, checks },
      verdict: {
        state: botState,
        verdict: botVerdict,
        findingsCount,
        findingKeys: keys,
        mustFix: nil(verdict.must_fix, []),
        commentUrl: nil(botComment?.url, null),
        commentId: nil(botComment?.id, null),
        degraded,
        degradedReason,
        sameRunAsLastTick: sameRun
      },
      threads: {
        total: snap.threads.length,
        unresolved,
        actionable,
        fixing,
        staleUnresolved,
        skippedOutdated,
        flaggedInjection
      },
      conversation: { actionable: convActionable, fixing: convFixing },
      reviews: { actionable: reviewActionable, fixing: reviewFixing },
      fixer,
      recurrence: { streak, lastRoundHadRejection, recurringKeys, fixAttempts },
      backoff: { idleStreak, intervalMinutes, waitSeconds },
      errors: [],
      snapshotPath,
      statePath
    }
  };
}

// plugins/pr-babysit/hooks/src/babysit-tick.ts
runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "babysit-tick.sh", [
    "--repo",
    "--pr",
    "--state-file",
    "--snapshot-out",
    "--snapshot-in",
    "--page-size",
    "--timeout",
    "--now"
  ]);
  const repo = flag(flags, "--repo");
  const numberText = flag(flags, "--pr");
  const stateFile = flag(flags, "--state-file");
  const snapshotIn = flag(flags, "--snapshot-in");
  const snapshotOut = flag(flags, "--snapshot-out") || `${stateFile.replace(/\.json$/, "")}.snapshot.json`;
  const pageText = flag(flags, "--page-size");
  const timeoutText = flag(flags, "--timeout");
  const now = flag(flags, "--now") || utcNow();
  if (!/^[^/\s]+\/[^/\s]+$/.test(repo))
    fail("usage", "babysit-tick.sh: --repo <owner/repo> required");
  if (!/^\d+$/.test(numberText))
    fail("usage", "babysit-tick.sh: --pr <n> required");
  if (!stateFile)
    fail("usage", "babysit-tick.sh: --state-file <path> required");
  if (pageText && !/^\d+$/.test(pageText))
    fail("usage", "babysit-tick.sh: --page-size must be an integer");
  if (timeoutText && !/^\d+$/.test(timeoutText))
    fail("usage", "babysit-tick.sh: --timeout must be an integer");
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(now))
    fail("usage", "babysit-tick.sh: --now must be YYYY-MM-DDTHH:MM:SSZ");
  if (!snapshotIn && !Bun.which("gh"))
    fail("gh_unavailable", "gh is required");
  const number = Number(numberText);
  const lock = new SlotLock(stateFile);
  lock.acquire();
  try {
    let previous = null;
    if (existsSync2(stateFile)) {
      try {
        previous = JSON.parse(readFileSync2(stateFile, "utf8"));
      } catch {
        fail("state_malformed", `babysit-tick.sh: state file is not valid JSON: ${stateFile}`, {
          source: "state"
        });
      }
      if (previous?.version !== 2)
        fail("state_malformed", `babysit-tick.sh: state file is not version 2: ${stateFile}`, {
          source: "state",
          version: previous?.version ?? null
        });
      if (previous.repo !== repo || previous.number !== number) {
        fail("slot_mismatch", `babysit-tick.sh: state file belongs to ${previous.repo}#${previous.number}, asked for ${repo}#${number}`, {
          source: "state",
          state: { repo: previous.repo, number: previous.number },
          requested: { repo, number }
        });
      }
    }
    const stampFailure = (error) => {
      if (previous === null)
        return;
      const code = error instanceof BabysitError ? error.code : "api_error";
      const message = error instanceof Error ? error.message : String(error);
      const pr = previous.pr;
      pr.lastError = { code, message, at: now };
      try {
        atomicWriteJson(stateFile, previous);
      } catch {
        process.stderr.write(`babysit-tick.sh: could not record lastError in ${stateFile}
`);
      }
    };
    let snapshot;
    let rawSnapshot;
    if (snapshotIn) {
      try {
        rawSnapshot = readFileSync2(snapshotIn, "utf8");
        snapshot = JSON.parse(rawSnapshot);
      } catch {
        fail("invalid_json", `babysit-tick.sh: --snapshot-in is not a JSON file: ${snapshotIn}`, {
          source: "snapshot"
        });
      }
      if (snapshot.repo !== repo || snapshot.number !== number) {
        fail("slot_mismatch", `babysit-tick.sh: --snapshot-in is for ${snapshot.repo}#${snapshot.number}, asked for ${repo}#${number}`, { source: "snapshot" });
      }
    } else {
      try {
        snapshot = await collectPr({
          repo,
          pr: number,
          ...pageText ? { pageSize: Number(pageText) } : {},
          ...timeoutText ? { timeoutSeconds: Number(timeoutText) } : {}
        });
      } catch (error) {
        stampFailure(error);
        throw error;
      }
    }
    let reduced;
    try {
      if (snapshot.version !== 1 || typeof snapshot.repo !== "string" || typeof snapshot.number !== "number" || typeof snapshot.head?.sha !== "string" || !snapshot.pr || typeof snapshot.pr !== "object" || !Array.isArray(snapshot.threads)) {
        fail("invalid_json", "reduce-state.sh: snapshot is not a version-1 pr-babysit snapshot", {
          source: "snapshot"
        });
      }
      reduced = reduceState(snapshot, previous, now, stateFile, snapshotOut);
    } catch (error) {
      const failure = error instanceof BabysitError ? error : new BabysitError("invalid_json", `reduce-state.sh: reducer failed on ${snapshotIn || snapshotOut}`, { source: "reduce" });
      stampFailure(failure);
      throw failure;
    }
    try {
      atomicWriteJson(snapshotOut, snapshot, rawSnapshot);
    } catch {
      fail("invalid_json", `babysit-tick.sh: could not write ${snapshotOut}`, {
        source: "snapshot_out"
      });
    }
    try {
      atomicWriteJson(stateFile, reduced.state);
    } catch {
      fail("invalid_json", `babysit-tick.sh: could not write ${stateFile}`, { source: "state" });
    }
    return reduced.result;
  } finally {
    lock.release();
  }
});
