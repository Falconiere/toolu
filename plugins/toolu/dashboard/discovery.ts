// Multi-project discovery: walk the configured base dirs (depth-capped) for any
// repo holding a plan-ledger, and turn each ledger file into a DiscoveredProject.
// Never throws — unreadable dirs/files are skipped so one bad tree can't sink the
// scan. Active-vs-idle filtering lives in aggregate.ts; this returns everything.

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";

import type { DashboardConfig } from "./config.ts";

/** One discovered project = one branch ledger under one repo root. */
export interface DiscoveredProject {
  /** Stable short id: sha1(`${root}@${branchSlug}`). */
  id: string;
  /** Repo root (the dir whose .claude/tmp/plan-ledger holds this ledger). */
  root: string;
  /** Branch name from the ledger (falls back to the file slug). */
  branch: string;
  /** Display label `${basename(root)} · ${branch}`. */
  label: string;
  ledgerPath: string;
  ledgerMtimeMs: number;
}

const PRUNE = new Set([".git", "node_modules", ".hg", ".svn", ".claude"]);

/** True if `p` is an existing directory; false on any error. */
function isDir(p: string): boolean {
  try {
    return statSync(p).isDirectory();
  } catch {
    return false;
  }
}

/** Build a DiscoveredProject from a ledger file path under `root`. */
function makeProject(root: string, ledgerPath: string): DiscoveredProject {
  const branchSlug = basename(ledgerPath, ".json");
  let branch = branchSlug;
  try {
    const parsed = JSON.parse(readFileSync(ledgerPath, "utf8")) as { branch?: unknown };
    if (typeof parsed.branch === "string" && parsed.branch.length > 0) branch = parsed.branch;
  } catch (err) {
    // The ledger was just listed by readdirSync, so a read/parse failure here is
    // corruption or a race — not the ordinary "no ledger" case. Keep the
    // slug-derived branch so the project still lists, but say why.
    console.error(`dashboard: reading branch from ${ledgerPath} failed — using the slug (${String(err)})`);
  }
  let ledgerMtimeMs = 0;
  try {
    ledgerMtimeMs = statSync(ledgerPath).mtimeMs;
  } catch (err) {
    // Leave 0 — treated as oldest by the activity TTL.
    console.error(`dashboard: stat of ${ledgerPath} failed — treating as oldest (${String(err)})`);
  }
  const id = createHash("sha1").update(`${root}@${branchSlug}`).digest("hex").slice(0, 12);
  return { id, root, branch, label: `${basename(root)} · ${branch}`, ledgerPath, ledgerMtimeMs };
}

/** Collect ledgers at `dir` and recurse into children while depth budget remains. */
function collectFrom(dir: string, depth: number, maxDepth: number, out: DiscoveredProject[]): void {
  const pl = join(dir, ".claude", "tmp", "plan-ledger");
  if (isDir(pl)) {
    let entries: string[] = [];
    try {
      entries = readdirSync(pl);
    } catch {
      entries = [];
    }
    for (const f of entries) {
      if (f.endsWith(".json")) out.push(makeProject(dir, join(pl, f)));
    }
  }
  if (depth >= maxDepth) return;
  let children: import("node:fs").Dirent[] = [];
  try {
    children = readdirSync(dir, { withFileTypes: true });
  } catch (err) {
    // Unreadable directory (permissions, vanished mid-walk): stop descending
    // this branch. Skipping it silently would look identical to "no projects
    // here", so the walk reports what it could not read.
    console.error(`dashboard: cannot list ${dir} — skipping subtree (${String(err)})`);
    return;
  }
  for (const ent of children) {
    if (ent.isSymbolicLink() || !ent.isDirectory()) continue;
    if (PRUNE.has(ent.name)) continue;
    collectFrom(join(dir, ent.name), depth + 1, maxDepth, out);
  }
}

/** Walk every configured root (≤ cfg.scanDepth) and return all ledgers found.
 *  Nonexistent / non-dir roots are skipped. Order follows roots, then walk order. */
export function discoverProjects(cfg: DashboardConfig): DiscoveredProject[] {
  const out: DiscoveredProject[] = [];
  const seen = new Set<string>();
  for (const root of cfg.roots) {
    if (!isDir(root)) continue;
    collectFrom(root, 0, cfg.scanDepth, out);
  }
  // dedupe in case roots overlap (e.g. ~/Projects and ~/Projects/sub both listed)
  return out.filter((p) => {
    if (seen.has(p.ledgerPath)) return false;
    seen.add(p.ledgerPath);
    return true;
  });
}
