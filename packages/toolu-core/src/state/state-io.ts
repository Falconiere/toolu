/**
 * State-file I/O shared by the state layer (#255). Output uses jq's byte
 * format, a write replaces the target atomically, and a sidecar lock
 * serializes TypeScript writers.
 */
import { randomUUID } from "node:crypto";
import {
  closeSync,
  fsyncSync,
  openSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
  writeSync,
} from "node:fs";
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
 * Write `body` to a fresh temp file beside `file` (exclusive create, so a
 * planted symlink is refused), fsync it, then rename it over `file`. A reader
 * sees either the old document or the new one, never a torn one. Returns
 * false, with the temp file removed, when any step fails.
 */
export function writeAtomic(file: string, body: string): boolean {
  const tmp = `${file}.${randomUUID()}.tmp`;
  let fd: number | undefined;
  try {
    fd = openSync(tmp, "wx", 0o644);
    writeSync(fd, body);
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    renameSync(tmp, file);
    return true;
  } catch {
    if (fd !== undefined) closeSync(fd);
    rmSync(tmp, { force: true });
    return false;
  }
}

export type LockOptions = { timeoutMs?: number; staleMs?: number; warn?: Warn };

const LOCK_POLL_MS = 10;
const DEFAULT_LOCK_TIMEOUT_MS = 5000;
const DEFAULT_LOCK_STALE_MS = 10_000;

function isStale(lock: string, staleMs: number): boolean {
  try {
    return Date.now() - statSync(lock).mtimeMs > staleMs;
  } catch {
    return false;
  }
}

/** Take `<file>.lock`: "held", "busy" (another holder), or "unavailable" (cannot create it). */
function tryLock(lock: string): "held" | "busy" | "unavailable" {
  try {
    writeFileSync(lock, `${process.pid} ${new Date().toISOString()}\n`, { flag: "wx" });
    return "held";
  } catch (error) {
    return isErrno(error, "EEXIST") ? "busy" : "unavailable";
  }
}

/**
 * Run `fn` holding `<file>.lock`, so TypeScript read-merge-write cycles on one
 * state file never interleave. A lock left by a crashed writer (older than
 * `staleMs`) is broken. When the lock cannot be taken in `timeoutMs`, or
 * cannot be created at all, `fn` runs unlocked: `writeAtomic` still keeps the
 * file whole, and a lost merge beats a lost failure record.
 */
export function withLock<T>(file: string, fn: () => T, options: LockOptions = {}): T {
  const lock = `${file}.lock`;
  const deadline = Date.now() + (options.timeoutMs ?? DEFAULT_LOCK_TIMEOUT_MS);
  let state = tryLock(lock);
  while (state === "busy") {
    if (isStale(lock, options.staleMs ?? DEFAULT_LOCK_STALE_MS)) {
      rmSync(lock, { force: true });
    } else if (Date.now() >= deadline) {
      (options.warn ?? stderrWarn)(`state: lock ${lock} still held; writing without it`);
      break;
    } else {
      Bun.sleepSync(LOCK_POLL_MS);
    }
    state = tryLock(lock);
  }
  try {
    return fn();
  } finally {
    if (state === "held") rmSync(lock, { force: true });
  }
}
