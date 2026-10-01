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
function errorCode(error) {
  return error && typeof error === "object" && "code" in error && typeof error.code === "string" ? error.code : undefined;
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
        let created = false;
        try {
          mkdirSync(this.path);
          created = true;
        } catch (error) {
          if (errorCode(error) !== "EEXIST")
            throw error;
          if (!existsSync(this.path)) {
            if (attempt === 0)
              continue;
            fail("locked", "slot is held by another controller", { pid: null, since: null });
          }
        }
        if (created) {
          this.held = true;
          try {
            const { dev, ino } = statSync(this.path);
            this.identity = { dev, ino };
            writeFileSync(join(this.path, "pid"), `${process.pid}
`);
            writeFileSync(join(this.path, "since"), `${Math.floor(Date.now() / 1000)}
`);
            return;
          } catch (error) {
            if (errorCode(error) === "ENOENT")
              fail("locked", "slot is held by another controller", { pid: null, since: null });
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
      if (error instanceof BabysitError)
        throw error;
      const message = error instanceof Error ? error.message : String(error);
      fail("api_error", `slot lock failed: ${message}`, { source: "lock" });
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
    const completed = Promise.all([
      proc.exited,
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text()
    ]).then(([rc, stdout, stderr]) => ({ rc, stdout, stderr }));
    let timedOut = false;
    let deadline;
    let escalation;
    const timeout = new Promise((resolve) => {
      deadline = setTimeout(() => {
        timedOut = true;
        try {
          proc.kill();
        } catch {}
        escalation = setTimeout(() => {
          try {
            proc.kill(9);
          } catch {}
          resolve({ rc: 124, stdout: "", stderr: "" });
        }, 250);
      }, timeoutSeconds * 1000);
    });
    let result;
    try {
      result = await Promise.race([completed, timeout]);
    } finally {
      if (deadline)
        clearTimeout(deadline);
      if (escalation)
        clearTimeout(escalation);
    }
    const { stdout, stderr } = result;
    const rc = timedOut ? 124 : result.rc;
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
async function resolveThread(options) {
  const { statePath, thread } = options;
  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const actions = state.actions;
    const prior = actions.resolved[thread];
    if (prior?.confirmed === true)
      return {
        ok: true,
        thread,
        confirmed: true,
        attempts: 0,
        alreadyResolved: true,
        at: prior.at ?? null
      };
    const mutation = "mutation($threadId:ID!){ resolveReviewThread(input:{threadId:$threadId}){ thread{ id isResolved } } }";
    let last = null;
    for (let attempt = 1;attempt <= 3; attempt += 1) {
      let response;
      try {
        response = await ghRun(["api", "graphql", "-f", `threadId=${thread}`, "-f", `query=${mutation}`], options.timeoutSeconds === undefined ? {} : { timeoutSeconds: options.timeoutSeconds });
      } catch (error) {
        ghFailure(error, "resolve");
      }
      const payload = parseGhJson(response, "resolve");
      if (Array.isArray(payload.errors) && payload.errors.length > 0) {
        fail("invalid_json", "resolve-thread.sh: mutation response carried errors[]", {
          source: "resolve",
          errors: payload.errors
        });
      }
      const data = payload.data;
      const resolved = data?.resolveReviewThread;
      last = resolved?.thread ?? null;
      if (last !== null && typeof last === "object" && last.isResolved === true) {
        const { head } = statePr(state);
        actions.resolved[thread] = {
          confirmed: true,
          at: utcNow(),
          attempts: attempt,
          headSha: head
        };
        atomicWriteJson(statePath, state);
        return { ok: true, thread, confirmed: true, attempts: attempt };
      }
      process.stderr.write(`resolve-thread.sh: attempt ${attempt} returned isResolved=false for ${thread}; retrying
`);
      await Bun.sleep(1000);
    }
    fail("resolve_unconfirmed", `resolve-thread.sh: ${thread} still unresolved after 3 attempt(s)`, { thread, attempts: 3, lastResponse: last });
  } finally {
    lock.release();
  }
}

// plugins/pr-babysit/hooks/src/babysit-resolve-thread.ts
runCli(async () => {
  const flags = parseFlags(process.argv.slice(2), "resolve-thread.sh", [
    "--state-file",
    "--thread",
    "--timeout"
  ]);
  const statePath = flag(flags, "--state-file");
  const thread = flag(flags, "--thread");
  if (!statePath)
    fail("usage", "resolve-thread.sh: --state-file required");
  if (!/^[A-Za-z0-9_=-]+$/.test(thread))
    fail("usage", `resolve-thread.sh: --thread <graphqlId> required (got '${thread}')`);
  if (!Bun.which("gh"))
    fail("gh_unavailable", "gh is required");
  return resolveThread({
    statePath,
    thread,
    ...flag(flags, "--timeout") ? { timeoutSeconds: Number(flag(flags, "--timeout")) } : {}
  });
});
