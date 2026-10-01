#!/usr/bin/env bun
// @bun

// plugins/pr-babysit/hooks/src/babysit-reply-thread.ts
import { existsSync as existsSync2, statSync as statSync2 } from "fs";

// plugins/pr-babysit/hooks/src/babysit/common.ts
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
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
function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
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
  identity = null;
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
    process.on("SIGTERM", this.onTerm);
    process.on("SIGINT", this.onInt);
    try {
      mkdirSync(dirname(this.path), { recursive: true });
      for (let attempt = 0;attempt < 2; attempt += 1) {
        try {
          mkdirSync(this.path);
          this.held = true;
          const { dev, ino } = statSync(this.path);
          this.identity = { dev, ino };
          writeFileSync(join(this.path, "pid"), `${process.pid}
`);
          writeFileSync(join(this.path, "since"), `${Math.floor(Date.now() / 1000)}
`);
          return;
        } catch (error) {
          if (!existsSync(this.path)) {
            if (attempt === 0)
              continue;
            throw error;
          }
        }
        const pidText = readTextOrEmpty(join(this.path, "pid"));
        const sinceText = readTextOrEmpty(join(this.path, "since"));
        const pid = /^\d+$/.test(pidText) ? Number(pidText) : null;
        const since = /^\d+$/.test(sinceText) ? Number(sinceText) : null;
        const staleAfter = Number(process.env.PB_LOCK_STALE_SECONDS ?? "600");
        let incompleteAgeMs = 0;
        if (pid === null || since === null) {
          try {
            incompleteAgeMs = Date.now() - statSync(this.path).mtimeMs;
          } catch {
            if (attempt === 0)
              continue;
          }
        }
        const stale = pid === null || since === null ? incompleteAgeMs >= 1000 : !pidAlive(pid) || Math.floor(Date.now() / 1000) - since > staleAfter;
        if (attempt === 0 && stale) {
          process.stderr.write(`pr-babysit: reclaiming stale lock ${this.path} (pid ${pidText || "?"}, since ${sinceText || "?"})
`);
          rmSync(this.path, { recursive: true, force: true });
          continue;
        }
        fail("locked", "slot is held by another controller", { pid, since });
      }
    } catch (error) {
      this.release();
      throw error;
    }
  }
  release() {
    let owned = false;
    if (this.held && this.identity) {
      try {
        const { dev, ino } = statSync(this.path);
        owned = dev === this.identity.dev && ino === this.identity.ino;
      } catch {}
    }
    if (owned) {
      rmSync(this.path, { recursive: true, force: true });
    }
    this.held = false;
    this.identity = null;
    process.off("SIGTERM", this.onTerm);
    process.off("SIGINT", this.onInt);
  }
}
function loadState(path) {
  if (!existsSync(path))
    fail("state_malformed", `state file not found: ${path} (run babysit-tick.js first)`, {
      source: "state"
    });
  let value;
  try {
    value = readJson(path);
  } catch {
    fail("state_malformed", `state file is not valid JSON: ${path}`, { source: "state" });
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail("state_malformed", `state file is not a version-2 pr-babysit state: ${path}`, {
      source: "state",
      version: null
    });
  }
  const state = value;
  if (state.version !== 2 || typeof state.repo !== "string" || typeof state.number !== "number") {
    fail("state_malformed", `state file is not a version-2 pr-babysit state: ${path}`, {
      source: "state",
      version: state.version ?? null
    });
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9_-]*\/[A-Za-z0-9_][A-Za-z0-9._-]*$/.test(state.repo)) {
    fail("state_malformed", `state file repo is not owner/name: ${state.repo}`, {
      source: "state"
    });
  }
  if (!/^\d+$/.test(String(state.number))) {
    fail("state_malformed", `state file number is not an integer: ${state.number}`, {
      source: "state"
    });
  }
  return state;
}

// plugins/pr-babysit/hooks/src/babysit/writes.ts
import { mkdtempSync as mkdtempSync2, readFileSync as readFileSync2, rmSync as rmSync2, writeFileSync as writeFileSync2 } from "fs";
import { tmpdir } from "os";
import { join as join2 } from "path";

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

// plugins/pr-babysit/hooks/src/babysit/writes.ts
function statePr(state) {
  const pr = state.pr;
  return { repo: String(state.repo), number: Number(state.number), head: String(pr.headSha ?? "") };
}
function ghFailure(error, source) {
  if (error instanceof GhError) {
    fail("api_error", `gh ${source} failed after ${error.attempts} attempt(s): ${error.lastMessage || `rc ${error.lastRc}`}`, {
      source,
      attempts: error.attempts,
      class: error.classification,
      lastMessage: error.lastMessage
    });
  }
  throw error;
}
function parseGhJson(text, source) {
  let value;
  try {
    value = JSON.parse(text);
  } catch {
    fail("invalid_json", `${source}: response is not valid JSON`, { source });
  }
  if (value === null || typeof value !== "object" || Array.isArray(value))
    fail("invalid_json", `${source}: response is not an object`, { source });
  return value;
}
async function replyThread(options) {
  const { statePath, kind, bodyPath } = options;
  const key = kind === "thread" ? `thread:${options.thread}@${options.inReplyTo}` : kind === "conversation" ? `conversation:${options.commentId}` : `review:${options.reviewId}`;
  const body = readFileSync2(bodyPath, "utf8");
  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const actions = state.actions;
    const recorded = actions.replied[key];
    if (recorded !== undefined && recorded !== null)
      fail("duplicate_reply", `reply-thread.sh: a reply to ${key} is already recorded; not posting again`, { key, recorded });
    const { repo, number, head } = statePr(state);
    const endpoint = kind === "thread" ? `repos/${repo}/pulls/${number}/comments/${options.root}/replies` : `repos/${repo}/issues/${number}/comments`;
    const dir = mkdtempSync2(join2(tmpdir(), "pr-babysit-reply-"));
    let response;
    try {
      const payload = join2(dir, "payload.json");
      writeFileSync2(payload, JSON.stringify({ body }));
      response = await ghRun(["api", "--method", "POST", endpoint, "--input", payload], options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds });
    } catch (error) {
      ghFailure(error, "reply");
    } finally {
      rmSync2(dir, { recursive: true, force: true });
    }
    const reply = parseGhJson(response, "reply");
    if (typeof reply.id !== "number")
      fail("invalid_json", "reply-thread.sh: reply response carried no comment id", {
        source: "reply"
      });
    actions.replied[key] = {
      commentId: reply.id,
      url: reply.html_url ?? null,
      at: utcNow(),
      headSha: head,
      kind
    };
    atomicWriteJson(statePath, state);
    return { ok: true, key, commentId: reply.id, url: reply.html_url ?? null };
  } finally {
    lock.release();
  }
}

// plugins/pr-babysit/hooks/src/babysit-reply-thread.ts
runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "reply-thread.sh", [
    "--state-file",
    "--kind",
    "--thread",
    "--root-comment",
    "--in-reply-to",
    "--comment-id",
    "--review-id",
    "--body-file",
    "--timeout"
  ]);
  const statePath = flag(flags, "--state-file");
  const kind = flag(flags, "--kind");
  const bodyPath = flag(flags, "--body-file");
  const thread = flag(flags, "--thread");
  const root = flag(flags, "--root-comment");
  const inReplyTo = flag(flags, "--in-reply-to");
  const commentId = flag(flags, "--comment-id");
  const reviewId = flag(flags, "--review-id");
  if (!statePath)
    fail("usage", "reply-thread.sh: --state-file required");
  if (!bodyPath || !existsSync2(bodyPath))
    fail("usage", "reply-thread.sh: --body-file <existing file> required");
  const bodySize = statSync2(bodyPath).size;
  if (bodySize === 0)
    fail("usage", "reply-thread.sh: reply body is empty");
  if (bodySize > 65536)
    fail("usage", "reply-thread.sh: reply body exceeds GitHub's 65536-character limit");
  if (kind === "thread") {
    if (!/^[A-Za-z0-9_=-]+$/.test(thread))
      fail("usage", "reply-thread.sh: --thread <graphqlId> required for --kind thread");
    if (!/^\d+$/.test(root))
      fail("usage", "reply-thread.sh: --root-comment <databaseId> required for --kind thread");
    if (!/^\d+$/.test(inReplyTo))
      fail("usage", "reply-thread.sh: --in-reply-to <databaseId> required for --kind thread");
  } else if (kind === "conversation") {
    if (!/^\d+$/.test(commentId))
      fail("usage", "reply-thread.sh: --comment-id <id> required for --kind conversation");
  } else if (kind === "review") {
    if (!/^\d+$/.test(reviewId))
      fail("usage", "reply-thread.sh: --review-id <id> required for --kind review");
  } else {
    fail("usage", "reply-thread.sh: --kind must be thread, conversation or review");
  }
  if (!Bun.which("gh"))
    fail("gh_unavailable", "gh is required");
  return replyThread({
    statePath,
    kind,
    thread,
    root,
    inReplyTo,
    commentId,
    reviewId,
    bodyPath,
    ...flag(flags, "--timeout") ? { timeoutSeconds: Number(flag(flags, "--timeout")) } : {}
  });
});
