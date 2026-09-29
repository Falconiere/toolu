/**
 * State-file I/O shared by the state layer (#255). Output uses jq's byte
 * format, a write replaces the target atomically, and a sidecar lock
 * serializes TypeScript writers.
 */
import { randomUUID } from "node:crypto";
import { linkSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import type { LoadedConfig } from "../config/config-load.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";

export type Warn = (message: string) => void;

/**
 * What every state writer takes: the environment and host to resolve roots
 * against, a preloaded config (loaded from the root otherwise), a clock, and
 * a warning sink (stderr by default).
 */
export type StateOptions = {
  env?: HostEnv;
  host?: HostName;
  config?: LoadedConfig;
  now?: () => Date;
  warn?: Warn;
};

/** Default warning sink: stderr, as the bash libs print their breadcrumbs. */
export const stderrWarn: Warn = (message) => {
  console.error(message);
};

/**
 * `value` as jq prints it: `jq` pretty (2-space indent) or `jq -c` compact.
 * Past JSON.stringify, jq's one difference is escaping DEL. A raw U+007F can
 * only appear inside a string, so a global replace is exact.
 */
export function toJqJson(value: unknown, pretty: boolean): string {
  const json = pretty ? JSON.stringify(value, null, 2) : JSON.stringify(value);
  return json.replaceAll("\u007f", "\\u007f");
}

/** `date -u +%Y-%m-%dT%H:%M:%SZ`: second resolution, UTC. */
export function isoSeconds(date: Date): string {
  return `${date.toISOString().slice(0, 19)}Z`;
}

/** UTF-8 byte order, which is codepoint order: how jq compares strings. */
export function compareJqStrings(a: string, b: string): number {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}

function isErrno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

/**
 * Write `body` to a fresh temp file beside `file`, then rename it over `file`.
 * The temp file is created exclusively, so a planted symlink is refused. It
 * is created 0600, like bash's `mktemp` (and `mv` keeps that mode). A reader
 * sees either the old document or the new one, never a torn one. Returns
 * false, never throws, when any step fails.
 */
export function writeAtomic(file: string, body: string): boolean {
  const tmp = `${file}.${randomUUID()}.tmp`;
  try {
    writeFileSync(tmp, body, { flag: "wx", mode: 0o600 });
    renameSync(tmp, file);
    return true;
  } catch {
    try {
      rmSync(tmp, { force: true });
    } catch {
      // The name is unique, so a leftover temp file is litter, never state.
    }
    return false;
  }
}

export type LockOptions = { timeoutMs?: number; staleMs?: number; warn?: Warn };

const LOCK_POLL_MS = 10;
const DEFAULT_LOCK_TIMEOUT_MS = 5000;
/** Well under the timeout: a critical section is one read and one rename, milliseconds long. */
const DEFAULT_LOCK_STALE_MS = 2000;

function lockContent(lock: string): string | undefined {
  try {
    return readFileSync(lock, "utf8");
  } catch {
    return undefined;
  }
}

/** The lock names its holder's pid; a pid that no longer exists crashed. */
function holderDead(content: string): boolean {
  const pid = Number(content.split(" ")[0]);
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return false;
  } catch (error) {
    return isErrno(error, "ESRCH");
  }
}

/**
 * Break the lock if its holder is dead, or if it is older than `staleMs`. The
 * lock is claimed by an atomic rename, so exactly one waiter wins. If the
 * claimed file is not the holder that was judged stale (a fresh lock was
 * taken in between), it is linked back, so a live holder never loses its lock.
 */
function breakIfStale(lock: string, staleMs: number): void {
  const content = lockContent(lock);
  const stat = statSync(lock, { throwIfNoEntry: false });
  if (content === undefined || stat === undefined) return;
  if (!holderDead(content) && Date.now() - stat.mtimeMs <= staleMs) return;
  const claimed = `${lock}.${randomUUID()}.broken`;
  try {
    renameSync(lock, claimed);
  } catch {
    return; // Another waiter claimed it first.
  }
  if (lockContent(claimed) !== content) {
    try {
      linkSync(claimed, lock);
    } catch {
      // A newer holder already exists; the claimed one's release check will simply not match.
    }
  }
  rmSync(claimed, { force: true });
}

/** Take `<file>.lock`: "held", "busy" (another holder), or "unavailable" (cannot create it). */
function tryLock(lock: string, content: string): "held" | "busy" | "unavailable" {
  try {
    writeFileSync(lock, content, { flag: "wx", mode: 0o600 });
    return "held";
  } catch (error) {
    return isErrno(error, "EEXIST") ? "busy" : "unavailable";
  }
}

/**
 * Run `fn` holding `<file>.lock`, so TypeScript read-merge-write cycles on one
 * state file never interleave. A lock is broken when its holder's pid is gone,
 * or when it is older than `staleMs`. If the lock cannot be taken within
 * `timeoutMs`, or cannot be created at all, `fn` runs unlocked: `writeAtomic`
 * still keeps the file whole, and a lost merge beats a lost failure record.
 * On release, the lock is removed only if it still carries this call's token.
 */
export function withLock<T>(file: string, fn: () => T, options: LockOptions = {}): T {
  const lock = `${file}.lock`;
  const ours = `${String(process.pid)} ${randomUUID()}\n`;
  const deadline = Date.now() + (options.timeoutMs ?? DEFAULT_LOCK_TIMEOUT_MS);
  let state = tryLock(lock, ours);
  while (state === "busy") {
    if (Date.now() >= deadline) {
      (options.warn ?? stderrWarn)(`state: lock ${lock} still held; writing without it`);
      break;
    }
    breakIfStale(lock, options.staleMs ?? DEFAULT_LOCK_STALE_MS);
    state = tryLock(lock, ours);
    if (state === "busy") Bun.sleepSync(LOCK_POLL_MS);
  }
  try {
    return fn();
  } finally {
    if (state === "held" && lockContent(lock) === ours) rmSync(lock, { force: true });
  }
}
