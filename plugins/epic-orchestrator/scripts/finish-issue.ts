/** Tear down one sub-issue after its PR merged: exit the agent, snapshot and
 * remove the herdr worktree workspace, delete the local branch, mark the
 * record merged.
 *
 * Usage: bun finish-issue.ts <state-dir> <key> [--abandon]
 * Refuses unless the PR is MERGED, or --abandon is given (which keeps the
 * branch and refuses over dirty work it could not snapshot). */

import { existsSync, statSync } from "node:fs";
import { join } from "node:path";
import { snapshot } from "./checkpoint.ts";
import { readJson, run, writeJson } from "./common.ts";

const USAGE = "usage: finish-issue.ts <state-dir> <key> [--abandon]";
const AGENT_EXIT_POLLS = 10;
const LEFTOVER_LINES = 20;

class UsageError extends Error {}

type IssueRecord = Record<string, unknown> & {
  repo?: string;
  branch?: string;
  checkout?: string;
  worktree?: string;
  workspace_id?: string;
  agent?: string;
};

export type Finished = {
  key: string | null;
  stage: "merged" | "abandoned";
  worktree_removed: boolean;
  branch_deleted: boolean;
  wip_ref: string | null;
  leftover_files: string[];
};

/** Run a command for its exit status and stdout; a missing binary counts as failure. */
async function attempt(cmd: string[]): Promise<{ ok: boolean; out: string }> {
  try {
    const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "ignore" });
    const [out, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
    return { ok: code === 0, out };
  } catch {
    return { ok: false, out: "" };
  }
}

const str = (v: unknown): string =>
  typeof v === "string" || typeof v === "number" ? String(v) : "";

const isDir = (path: string): boolean =>
  path !== "" && existsSync(path) && statSync(path).isDirectory();

async function requireMerged(stateDir: string, key: string, repo: string): Promise<void> {
  const pr = str(readJson<Record<string, unknown>>(join(stateDir, "status", `${key}.json`), {}).pr);
  if (pr === "") throw new Error(`no PR recorded for ${key}; use --abandon to tear down anyway`);
  const state = (
    await run(["gh", "pr", "view", pr, "-R", repo, "--json", "state", "-q", ".state"])
  ).trim();
  if (state !== "MERGED") throw new Error(`PR ${repo}#${pr} is ${state}, not MERGED`);
}

/** Babysit's cron is session-scoped, so exiting the agent ends it too. */
async function exitAgent(agent: string): Promise<void> {
  if (agent === "" || !(await attempt(["herdr", "agent", "get", agent])).ok) return;
  await attempt(["herdr", "agent", "send-keys", agent, "esc"]);
  await attempt(["herdr", "agent", "prompt", agent, "/exit"]);
  for (let i = 0; i < AGENT_EXIT_POLLS; i++) {
    if (!(await attempt(["herdr", "agent", "get", agent])).ok) return;
    await Bun.sleep(1000);
  }
}

/** The snapshot ref when one holds the worktree's work, else null (the
 * --abandon refusal below then guards dirty work). */
async function snapshotRef(worktree: string, key: string): Promise<string | null> {
  try {
    const snap = await snapshot(worktree, key);
    return snap.changed || snap.sha ? snap.ref : null;
  } catch (err) {
    process.stderr.write(`WARN: snapshot of ${key} failed: ${String(err)}\n`);
    return null;
  }
}

/** `git status --porcelain`, first lines only, status columns kept. */
async function leftovers(worktree: string): Promise<string[]> {
  if (!isDir(worktree)) return [];
  const status = await attempt(["git", "-C", worktree, "status", "--porcelain"]);
  return status.out
    .split("\n")
    .filter((line) => line.length > 0)
    .slice(0, LEFTOVER_LINES);
}

export async function finishIssue(
  stateDir: string,
  key: string,
  abandon: boolean,
): Promise<Finished> {
  const path = join(stateDir, "issues", `${key}.json`);
  const rec = readJson<IssueRecord | null>(path, null);
  if (rec === null || typeof rec !== "object") throw new Error(`no record ${path}`);
  const worktree = str(rec.worktree);
  const workspace = str(rec.workspace_id);

  if (!abandon) await requireMerged(stateDir, key, str(rec.repo));
  await exitAgent(str(rec.agent));

  // Snapshot to refs/epic-wip/<key> first: it outlives the worktree removal below.
  const wipRef = await snapshotRef(worktree, key);
  const dirty = await leftovers(worktree);
  if (abandon && wipRef === null && dirty.length > 0) {
    throw new Error(`refusing --abandon: ${worktree} has uncommitted work and the snapshot failed`);
  }

  let removed = false;
  if (workspace === "") {
    process.stderr.write(`WARN: no workspace_id on record ${key}; skipping worktree remove\n`);
  } else {
    const force = abandon ? [] : ["--force"];
    removed = (await attempt(["herdr", "worktree", "remove", "--workspace", workspace, ...force]))
      .ok;
  }

  // Squash merges leave the branch unmerged in git's eyes, hence -D.
  const checkout = str(rec.checkout);
  const branch = str(rec.branch);
  const branchDeleted =
    !abandon && checkout !== "" && branch !== ""
      ? (await attempt(["git", "-C", checkout, "branch", "-D", branch])).ok
      : false;

  const stage = abandon ? "abandoned" : "merged";
  const finishedAt = `${new Date().toISOString().slice(0, 19)}Z`;
  await writeJson(path, {
    ...rec,
    stage,
    finished_at: finishedAt,
    worktree_removed: removed,
    branch_deleted: branchDeleted,
    wip_ref: wipRef,
    leftover_files: dirty,
  });
  return {
    key: typeof rec.key === "string" ? rec.key : null,
    stage,
    worktree_removed: removed,
    branch_deleted: branchDeleted,
    wip_ref: wipRef,
    leftover_files: dirty,
  };
}

function parseArgs(argv: string[]): { stateDir: string; key: string; abandon: boolean } {
  const [stateDir, key, mode, ...extra] = argv;
  if (!stateDir || !key || extra.length > 0 || (mode !== undefined && mode !== "--abandon")) {
    throw new UsageError(USAGE);
  }
  return { stateDir, key, abandon: mode === "--abandon" };
}

async function main(): Promise<void> {
  const { stateDir, key, abandon } = parseArgs(process.argv.slice(2));
  process.stdout.write(`${JSON.stringify(await finishIssue(stateDir, key, abandon))}\n`);
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(`${err instanceof Error ? err.message : String(err)}\n`);
    process.exit(err instanceof UsageError ? 2 : 1);
  });
}
