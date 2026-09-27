/** Snapshot a worker's worktree so no progress is lost to a crash, a usage
 * limit, a host switch, or a teardown.
 *
 * The snapshot is a commit of the full working tree (tracked, staged, and
 * untracked-but-not-ignored files) on top of HEAD, built with a throwaway
 * index so the worker's own index and files are never touched. It is stored
 * at `refs/epic-wip/<key>` in the shared repository, with a reflog, so it
 * outlives the worktree. Restore: `git checkout -b recover refs/epic-wip/<key>`. */

import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { readJson } from "./common.ts";

export type Snapshot = {
  key: string;
  ref: string;
  sha: string | null;
  dirty: boolean;
  unpushed: number;
  changed: boolean;
  skipped?: string;
};

async function git(
  cwd: string,
  args: string[],
  env: Record<string, string> = {},
): Promise<{ out: string; ok: boolean }> {
  const proc = Bun.spawn(["git", "-C", cwd, ...args], {
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, ...env },
  });
  const [out, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
  return { out: out.trim(), ok: code === 0 };
}

const IDENTITY = {
  GIT_AUTHOR_NAME: "epic-orchestrator",
  GIT_AUTHOR_EMAIL: "epic-orchestrator@localhost",
  GIT_COMMITTER_NAME: "epic-orchestrator",
  GIT_COMMITTER_EMAIL: "epic-orchestrator@localhost",
};

export async function snapshot(worktree: string, key: string): Promise<Snapshot> {
  const ref = `refs/epic-wip/${key}`;
  const base: Snapshot = { key, ref, sha: null, dirty: false, unpushed: 0, changed: false };
  if (!worktree || !existsSync(join(worktree, ".git"))) return { ...base, skipped: "no worktree" };
  const head = await git(worktree, ["rev-parse", "HEAD"]);
  if (!head.ok) return { ...base, skipped: "no HEAD" };
  const dirty = (await git(worktree, ["status", "--porcelain"])).out !== "";
  const ahead = await git(worktree, ["rev-list", "--count", "HEAD", "--not", "--remotes=origin"]);
  const unpushed = ahead.ok ? Number(ahead.out) || 0 : 0;
  if (!dirty && unpushed === 0) return { ...base, skipped: "nothing at risk" };
  let sha = head.out;
  if (dirty) {
    const dir = mkdtempSync(join(tmpdir(), "epic-wip-"));
    const env = { GIT_INDEX_FILE: join(dir, "index"), ...IDENTITY };
    try {
      const steps = [
        await git(worktree, ["read-tree", "HEAD"], env),
        await git(worktree, ["add", "-A"], env),
      ];
      const tree = await git(worktree, ["write-tree"], env);
      if (steps.some((s) => !s.ok) || !tree.ok)
        return { ...base, dirty, unpushed, skipped: "git failed" };
      const stamp = new Date().toISOString();
      const commit = await git(
        worktree,
        ["commit-tree", tree.out, "-p", head.out, "-m", `epic-wip ${key} ${stamp}`],
        env,
      );
      if (!commit.ok) return { ...base, dirty, unpushed, skipped: "commit-tree failed" };
      sha = commit.out;
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
  const current = await git(worktree, ["rev-parse", "--verify", "--quiet", ref]);
  // Same tree as the last snapshot: keep the reflog quiet.
  if (current.ok) {
    const same = await git(worktree, ["diff", "--quiet", current.out, sha]);
    if (same.ok) return { ...base, sha: current.out, dirty, unpushed };
  }
  const upd = await git(worktree, [
    "update-ref",
    "--create-reflog",
    "-m",
    "epic checkpoint",
    ref,
    sha,
  ]);
  if (!upd.ok) return { ...base, dirty, unpushed, skipped: "update-ref failed" };
  return { ...base, sha, dirty, unpushed, changed: true };
}

type Rec = { stage?: string; worktree?: string };

export async function snapshotActive(stateDir: string, only?: string): Promise<Snapshot[]> {
  let names: string[] = [];
  try {
    names = readdirSync(join(stateDir, "issues")).filter((n) => n.endsWith(".json"));
  } catch {
    return [];
  }
  const out: Snapshot[] = [];
  for (const name of names) {
    const key = name.slice(0, -5);
    if (only !== undefined && key !== only) continue;
    const rec = readJson<Rec>(join(stateDir, "issues", name), {});
    const active = rec.stage === "running" || rec.stage === "awaiting_merge";
    if (only === undefined && !active) continue;
    out.push(await snapshot(rec.worktree ?? "", key));
  }
  return out;
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let stateDir: string | undefined;
  let key: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--state-dir") stateDir = argv[++i];
    else if (a === "--key") key = argv[++i];
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!stateDir) throw new Error("usage: checkpoint.ts --state-dir DIR [--key KEY]");
  process.stdout.write(JSON.stringify(await snapshotActive(stateDir, key), null, 2) + "\n");
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
