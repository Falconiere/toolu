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

// plugins/pr-babysit/hooks/src/babysit-record.ts
runCli(() => {
  const [sub = "", ...args] = process.argv.slice(2);
  const flags = parseFlags(args, "record.sh", ["--state-file", "--thread", "--had-rejection", "--status"], ["--fix-pushed"]);
  const statePath = flag(flags, "--state-file");
  const thread = flag(flags, "--thread");
  const rejection = flag(flags, "--had-rejection");
  const status = flag(flags, "--status");
  if (!statePath)
    fail("usage", "record.sh: --state-file required");
  if (sub === "flag-injection") {
    if (!thread)
      fail("usage", "record.sh flag-injection: --thread <graphqlId> required");
  } else if (sub === "round") {
    if (rejection !== "true" && rejection !== "false")
      fail("usage", "record.sh round: --had-rejection true|false required");
  } else if (sub === "status") {
    if (!["complete", "escalated", "cancelled"].includes(status))
      fail("usage", "record.sh status: --status complete|escalated|cancelled required");
  } else {
    fail("usage", "record.sh: subcommand must be flag-injection, round or status");
  }
  const lock = new SlotLock(statePath);
  lock.acquire();
  try {
    const state = loadState(statePath);
    const now = utcNow();
    if (sub === "flag-injection") {
      const actions = state.actions;
      actions.flagged[thread] = { reason: "injection", at: now };
      atomicWriteJson(statePath, state);
      return { ok: true, recorded: "flag-injection", thread, at: now };
    }
    if (sub === "round") {
      const pr = state.pr;
      const hadRejection = rejection === "true";
      const fixPushed = flags["--fix-pushed"] === true;
      pr.lastRoundFindingKeys = pr.botFindingKeys ?? [];
      pr.lastRoundHadRejection = hadRejection;
      pr.fixAttempts = fixPushed ? Math.min(Number(pr.fixAttempts ?? 0) + 1, 5) : pr.fixAttempts ?? 0;
      state.lastRound = { at: now, hadRejection, fixPushed, headSha: pr.headSha ?? null };
      const fixer = state.fixer;
      if (fixer !== null && fixer !== undefined && fixer.status !== "running" && fixer.status !== "blocked")
        state.fixer = null;
      atomicWriteJson(statePath, state);
      return {
        ok: true,
        recorded: "round",
        lastRoundFindingKeys: pr.lastRoundFindingKeys,
        lastRoundHadRejection: pr.lastRoundHadRejection,
        fixAttempts: pr.fixAttempts
      };
    }
    state.status = status;
    state.statusChangedAt = now;
    atomicWriteJson(statePath, state);
    return { ok: true, recorded: "status", status, at: now };
  } finally {
    lock.release();
  }
});
