// Resolve the ledger path and repo root by shelling the real plan-ledger.sh, so
// the dashboard never duplicates branch-slug or project-root logic. Pure helpers
// with no server state; index.ts calls these on startup and on each poll.

import { spawnSync } from "node:child_process";
import { join } from "node:path";

const ENGINE = join(import.meta.dir, "..", "hooks", "lib", "plan-ledger.sh");

/** Run `plan-ledger.sh <sub>` and return trimmed stdout, or null on any failure. */
function engine(sub: "path" | "root"): string | null {
  const res = spawnSync("bash", [ENGINE, sub], { encoding: "utf8" });
  if (res.status !== 0 || res.error) return null;
  const out = res.stdout.trim();
  return out.length > 0 ? out : null;
}

/** Current branch's ledger path (re-resolve each poll to pick up branch switches). */
export function resolveLedgerPath(): string | null {
  return engine("path");
}

/** Repo root for `git -C <root>` (detect_project_root). */
export function resolveRepoRoot(): string | null {
  return engine("root");
}
