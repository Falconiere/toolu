/** Map owner/repo -> local primary checkout path (read-only). */

import { existsSync } from "node:fs";
import { join } from "node:path";
import { CommandError, herdr } from "./common.ts";

const REMOTE = /github\.com[:/]([^/]+)\/(.+?)(?:\.git)?\/?$/;

export function originOf(path: string): string | null {
  const proc = Bun.spawnSync(["git", "-C", path, "remote", "get-url", "origin"]);
  if (proc.exitCode !== 0) return null;
  const m = REMOTE.exec(proc.stdout.toString().trim());
  return m && m[1] && m[2] ? `${m[1]}/${m[2]}`.toLowerCase() : null;
}

type HerdrWorkspace = {
  worktree?: {
    repo_root?: string;
    is_linked_worktree?: boolean;
  };
};

export async function herdrRoots(): Promise<string[]> {
  try {
    const result = await herdr(["workspace", "list"]);
    const workspaces = (result.workspaces as HerdrWorkspace[] | undefined) ?? [];
    const roots: string[] = [];
    for (const ws of workspaces) {
      const wt = ws.worktree ?? {};
      if (wt.repo_root && !wt.is_linked_worktree) {
        roots.push(wt.repo_root);
      }
    }
    return roots;
  } catch (err) {
    if (err instanceof CommandError) return [];
    throw err;
  }
}

function uniquePaths(paths: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of paths) {
    if (!seen.has(p)) {
      seen.add(p);
      out.push(p);
    }
  }
  return out;
}

export async function resolveCheckouts(repos: string[]): Promise<Record<string, string | null>> {
  const wanted = new Map(repos.map((r) => [r.toLowerCase(), r]));
  const found: Record<string, string | null> = Object.fromEntries(repos.map((r) => [r, null]));
  const known = await herdrRoots();
  const candidates = uniquePaths(known);
  const parents = uniquePaths([
    ...known.map((p) => join(p, "..")),
    ...(process.env.EPIC_REPO_ROOTS?.split(":").filter(Boolean) ?? []),
  ]);
  const names = new Set(repos.map((r) => r.split("/", 2)[1] ?? ""));
  for (const parent of parents) {
    for (const name of names) {
      if (name) candidates.push(join(parent, name));
    }
  }
  for (const path of candidates) {
    if (!existsSync(join(path, ".git"))) continue;
    const origin = originOf(path);
    if (origin === null) continue;
    const canonical = wanted.get(origin);
    if (canonical !== undefined && found[canonical] === null) {
      found[canonical] = path;
    }
  }
  return found;
}
