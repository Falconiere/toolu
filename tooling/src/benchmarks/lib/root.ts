/**
 * Where the harness lives: the repo root (git toplevel, else the nearest
 * ancestor holding .git) and the results directory (`BENCH_RESULTS_DIR`, else
 * benchmarks/results).
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { envOr } from "../../env.ts";

export function benchRoot(from: string = import.meta.dir): string {
  const res = spawnSync("git", ["-C", from, "rev-parse", "--show-toplevel"], { encoding: "utf8" });
  if (res.status === 0) return res.stdout.trim();
  for (let dir = from; dir !== "/"; dir = dirname(dir)) {
    if (existsSync(join(dir, ".git"))) return dir;
  }
  throw new Error("benchmarks: cannot locate repo root");
}

export function resultsDir(
  root: string,
  env: Record<string, string | undefined> = process.env,
): string {
  return envOr("BENCH_RESULTS_DIR", join(root, "benchmarks/results"), env);
}

/** HEAD of the repo, or "unknown" outside a work tree. */
export function headCommit(root: string): string {
  const res = spawnSync("git", ["-C", root, "rev-parse", "HEAD"], { encoding: "utf8" });
  return res.status === 0 ? res.stdout.trim() : "unknown";
}

/** Today in local time, YYYY-MM-DD (the `date +%Y-%m-%d` of the bash harness). */
export function today(now: Date = new Date()): string {
  return localDay(now);
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export function localDay(date: Date): string {
  return `${String(date.getFullYear())}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
