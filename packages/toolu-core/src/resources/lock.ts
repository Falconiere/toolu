/** Exclusive local ownership; stale reclamation is itself serialized. */
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { randomUUID } from "node:crypto";
import { isJsonObject } from "../config/config-load.ts";

export function errno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

export function processAlive(pid: number): boolean {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (errno(error, "ESRCH")) return false;
    throw error;
  }
}

export function readJsonFile(path: string, fallback: unknown): unknown {
  try {
    const value: unknown = JSON.parse(readFileSync(path, "utf8"));
    return value;
  } catch (error) {
    if (errno(error, "ENOENT")) return fallback;
    throw error;
  }
}

export function writeJsonAtomic(path: string, value: unknown): void {
  mkdirSync(dirname(path), { recursive: true });
  const temp = `${path}.${process.pid}.${randomUUID()}.tmp`;
  try {
    writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx", mode: 0o600 });
    renameSync(temp, path);
  } finally {
    rmSync(temp, { force: true });
  }
}

export type LockHandle = { token: string; release: () => void };

function stale(path: string): boolean {
  const owner = readJsonFile(`${path}/owner.json`, null);
  if (isJsonObject(owner) && typeof owner.pid === "number" && typeof owner.token === "string")
    return !processAlive(owner.pid);
  // Unknown/corrupt ownership requires explicit reconciliation.
  return false;
}

function reclaim(path: string): void {
  const reap = `${path}.reap`;
  try {
    mkdirSync(reap);
  } catch (error) {
    if (errno(error, "EEXIST")) return;
    throw error;
  }
  try {
    if (stale(path)) rmSync(path, { recursive: true });
  } finally {
    rmSync(reap, { recursive: true });
  }
}

/** A crashed reclaimer leaves .reap: fail closed until operator reconciliation. */
export async function acquireLock(
  path: string,
  opts: { timeoutMs?: number } = {},
): Promise<LockHandle | null> {
  const timeoutMs = opts.timeoutMs ?? 0;
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0 || timeoutMs > 2_147_483_647) {
    throw new Error("lock timeoutMs must be a non-negative finite number within the timer range");
  }
  mkdirSync(dirname(path), { recursive: true });
  const until = Date.now() + timeoutMs;
  const attempt = async (): Promise<LockHandle | null> => {
    const token = randomUUID();
    const prepared = `${path}.${token}.pending`;
    try {
      if (existsSync(`${path}.reap`)) return null;
      if (existsSync(path)) {
        reclaim(path);
        if (existsSync(path)) {
          if (Date.now() >= until) return null;
          return Bun.sleep(25).then(attempt);
        }
      }
      mkdirSync(prepared);
      writeJsonAtomic(`${prepared}/owner.json`, { pid: process.pid, token });
      // Existing locks contain owner.json, so rename cannot replace them.
      renameSync(prepared, path);
      return {
        token,
        release: () => {
          const owner = readJsonFile(`${path}/owner.json`, null);
          if (isJsonObject(owner) && owner.token === token) {
            rmSync(path, { recursive: true });
          }
        },
      };
    } catch (error) {
      if (!errno(error, "EEXIST") && !errno(error, "ENOTEMPTY")) throw error;
    } finally {
      rmSync(prepared, { recursive: true, force: true });
    }
    reclaim(path);
    if (Date.now() >= until) return null;
    return Bun.sleep(25).then(attempt);
  };
  return attempt();
}

export async function withResourceLock<T>(root: string, fn: () => T | Promise<T>): Promise<T> {
  const lock = await acquireLock(`${root}/state.lock`, { timeoutMs: 5000 });
  if (!lock) throw new Error("resource state busy; retry later");
  try {
    return await fn();
  } finally {
    lock.release();
  }
}
