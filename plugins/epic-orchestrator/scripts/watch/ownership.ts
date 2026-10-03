/** Exclusive watcher ownership, including migration from the legacy file lock. */

import { lstatSync, rmSync } from "node:fs";
import { join } from "node:path";
import { acquireLock } from "../../hooks/dist/epic-runtime.js";
import { readJson } from "../common.ts";

type LegacyLock = { pid: number };

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return !(error instanceof Error && "code" in error && error.code === "ESRCH");
  }
}

/** One watcher per epic; a second watcher would duplicate every event. */
export async function takeWatchLock(state: string) {
  const path = join(state, "watch.lock");
  try {
    if (lstatSync(path).isFile()) {
      const held = readJson<LegacyLock | null>(path, null);
      if (!held || !Number.isSafeInteger(held.pid) || alive(held.pid)) return null;
      rmSync(path);
    }
  } catch (error) {
    const missing = error instanceof Error && "code" in error && error.code === "ENOENT";
    if (!missing) throw error;
  }
  return acquireLock(path);
}
