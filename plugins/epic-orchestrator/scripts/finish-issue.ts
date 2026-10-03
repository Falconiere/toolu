/** Tear down one sub-issue after its PR merged: exit the agent, snapshot and
 * remove the herdr worktree workspace, delete the local branch, mark the
 * record merged.
 *
 * Usage: bun finish-issue.ts <state-dir> <key> [--abandon]
 * Refuses unless the PR is MERGED, or --abandon is given (which keeps the
 * branch and refuses over dirty work it could not snapshot). */

import { existsSync, statSync } from "node:fs";
import { join, normalize } from "node:path";
import { reconcileResourceJobs } from "../hooks/dist/epic-runtime.js";
import { activeJobs } from "../hooks/dist/epic-runtime.js";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import { snapshot } from "./checkpoint.ts";
import { herdr, readJson, run, writeJson } from "./common.ts";
import { inspectAgent, shutdownAgent } from "./lifecycle.ts";
import { hostKind } from "./hosts.ts";
import { ownedWorkload, verifyOwnedExit } from "./workload.ts";
import { withLaunchOwnership } from "./admission.ts";
import {
  fenceCleanupOwnership,
  readIssueRecord,
  releaseRecordedOwnership,
  removalIntent,
  type CleanupOwnership,
  type IssueRecord,
  type RemovalIntent,
} from "./finish-state.ts";

const USAGE = "usage: finish-issue.ts <state-dir> <key> [--abandon]";
const LEFTOVER_LINES = 20;

class UsageError extends Error {}

type Finished = {
  key: string | null;
  stage: "merged" | "abandoned" | "cleanup-incomplete";
  worktree_removed: boolean;
  branch_deleted: boolean;
  wip_ref: string | null;
  leftover_files: string[];
};

/** Run a command for its exit status and stdout; a missing binary counts as failure.
 * `stderr: "inherit"` surfaces the tool's own error where the operator sees it. */
async function attempt(
  cmd: string[],
  stderr: "ignore" | "inherit" = "ignore",
): Promise<{ ok: boolean; out: string }> {
  try {
    const result = await runCommand(cmd, { timeoutMs: 30_000 });
    if (stderr === "inherit") process.stderr.write(result.stderr);
    return {
      ok: result.exitCode === 0 && !result.timedOut && !result.truncated,
      out: result.stdout,
    };
  } catch (error) {
    process.stderr.write(`${String(error)}\n`);
    return { ok: false, out: "" };
  }
}

const str = (v: unknown): string =>
  typeof v === "string" || typeof v === "number" ? String(v) : "";

const isDir = (path: string): boolean =>
  path !== "" && existsSync(path) && statSync(path).isDirectory();

function finished(rec: IssueRecord): Finished {
  return {
    key: rec.key,
    stage: rec.stage as Finished["stage"],
    worktree_removed: rec.worktree_removed === true,
    branch_deleted: rec.branch_deleted === true,
    wip_ref: typeof rec.wip_ref === "string" ? rec.wip_ref : null,
    leftover_files: Array.isArray(rec.leftover_files)
      ? rec.leftover_files.filter((entry): entry is string => typeof entry === "string")
      : [],
  };
}

function terminalCleanupComplete(rec: IssueRecord): boolean {
  if (rec.worktree_removed !== true || existsSync(rec.worktree)) return false;
  return rec.stage === "abandoned" || rec.branch_deleted === true;
}

/** The PR number lives in the worker's status file (`status/<key>.json`), the
 * only place anything records it: `report.ts --pr N` writes it there, while the
 * issue record written by launch-issue.ts and merge-gate.ts carries no `pr`. */
async function requireMerged(stateDir: string, key: string, repo: string): Promise<void> {
  const pr = str(readJson<Record<string, unknown>>(join(stateDir, "status", `${key}.json`), {}).pr);
  if (pr === "") throw new Error(`no PR recorded for ${key}; use --abandon to tear down anyway`);
  const state = (
    await run(["gh", "pr", "view", pr, "-R", repo, "--json", "state", "-q", ".state"])
  ).trim();
  if (state !== "MERGED") throw new Error(`PR ${repo}#${pr} is ${state}, not MERGED`);
}

/** Babysit's cron is session-scoped, so exiting the agent ends it too. */
async function exitAgent(rec: IssueRecord): Promise<void> {
  const agent = str(rec.agent);
  if (!agent) return;
  if (!str(rec.pane_id) || !str(rec.worktree))
    throw new Error("cannot verify agent ownership without pane and worktree");
  const outcome = await shutdownAgent(agent, {
    kind: hostKind(str(rec.kind) || "claude"),
    pane: str(rec.pane_id),
    cwd: str(rec.worktree),
  });
  if (outcome !== "exited") throw new Error(`agent ${agent} shutdown incomplete`);
}

/** The snapshot ref when one holds the worktree's work, else null (the
 * --abandon refusal below then guards dirty work). */
async function snapshotRef(worktree: string, key: string): Promise<string | null> {
  try {
    const snap = await snapshot(worktree, key);
    if (snap.skipped && snap.skipped !== "nothing at risk")
      throw new Error(`required checkpoint failed: ${snap.skipped}`);
    return snap.changed || snap.sha ? snap.ref : null;
  } catch (err) {
    process.stderr.write(`WARN: snapshot of ${key} failed: ${String(err)}\n`);
    throw err;
  }
}

/** `git status --porcelain`, first lines only, status columns kept. */
async function leftovers(worktree: string): Promise<string[]> {
  if (!isDir(worktree)) return [];
  const status = await attempt(["git", "-C", worktree, "status", "--porcelain"]);
  if (!status.ok) throw new Error(`cannot inspect worktree ${worktree}`);
  return status.out
    .split("\n")
    .filter((line) => line.length > 0)
    .slice(0, LEFTOVER_LINES);
}

async function finishIssue(stateDir: string, key: string, abandon: boolean): Promise<Finished> {
  const path = join(stateDir, "issues", `${key}.json`);
  const rec = readIssueRecord(path, key);
  const terminalStage = rec.stage === "merged" || rec.stage === "abandoned";
  if (terminalStage && terminalCleanupComplete(rec)) {
    await releaseRecordedOwnership(rec, stateDir);
    return finished(rec);
  }
  const effectiveAbandon = abandon || rec.stage === "abandoned";
  if (terminalStage) {
    Object.assign(rec, {
      stage: "cleanup-incomplete",
      finished_at: undefined,
      cleanup_error: "terminal record lacks verified cleanup evidence",
    });
    await writeJson(path, rec);
  }
  const worktree = rec.worktree;

  if (!effectiveAbandon) await requireMerged(stateDir, key, rec.repo);
  let saved: IssueRecord = rec;
  try {
    const ownership = await fenceCleanupOwnership(path, stateDir, key, rec);
    const absent = !existsSync(worktree);
    const intent = removalIntent(rec);
    const removedBefore = rec.worktree_removed === true && absent;
    const preparedRemoval = rec.worktree_removed !== true && absent && intent !== null;
    if ((removedBefore || preparedRemoval) && ownership === null)
      throw new Error("cannot verify legacy cleanup ownership after worktree removal");
    let result: CleanupResult;
    if (preparedRemoval && intent && ownership !== null) {
      await verifyPreparedRemoval(rec, intent, ownership);
      result = await finishRemovedWorktree(
        path,
        rec,
        intent.wip_ref,
        intent.leftover_files,
        effectiveAbandon,
      );
    } else if (removedBefore) {
      result = await finishAfterRemoval(rec, ownership, effectiveAbandon);
    } else {
      result = await finishLiveWorktree(path, rec, ownership, effectiveAbandon);
    }
    const stage =
      result.removed && (effectiveAbandon || result.branchDeleted)
        ? effectiveAbandon
          ? "abandoned"
          : "merged"
        : "cleanup-incomplete";
    const finishedAt = `${new Date().toISOString().slice(0, 19)}Z`;
    saved = {
      ...rec,
      stage,
      ...(stage === "cleanup-incomplete"
        ? { cleanup_attempt_at: finishedAt, finished_at: undefined }
        : { finished_at: finishedAt, cleanup_error: undefined }),
      worktree_removed: result.removed,
      branch_deleted: result.branchDeleted,
      wip_ref: result.wipRef,
      leftover_files: result.dirty,
    };
    if (stage !== "cleanup-incomplete") delete saved.removal_intent;
    await writeJson(path, saved);
    if (stage !== "cleanup-incomplete") await releaseRecordedOwnership(saved, stateDir);
    return finished(saved);
  } catch (error) {
    await writeJson(path, {
      ...saved,
      stage: "cleanup-incomplete",
      finished_at: undefined,
      cleanup_error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
}

type CleanupResult = {
  removed: boolean;
  branchDeleted: boolean;
  wipRef: string | null;
  dirty: string[];
};

async function finishLiveWorktree(
  path: string,
  rec: IssueRecord,
  ownership: CleanupOwnership | null,
  abandon: boolean,
): Promise<CleanupResult> {
  const worktree = rec.worktree;
  const workspace = str(rec.workspace_id);
  let shellPid: number | undefined;
  if (typeof rec.pane_id === "string") {
    const result = await herdr(["pane", "process-info", "--pane", rec.pane_id], 10_000);
    const info = result.process_info as { shell_pid?: number } | undefined;
    if (typeof info?.shell_pid !== "number") throw new Error("pane shell ownership unavailable");
    shellPid = info.shell_pid;
  }
  const owned = await ownedWorkload(worktree, shellPid);
  await exitAgent(rec);
  if (!(await verifyOwnedExit(owned)) || (await ownedWorkload(worktree, shellPid)).length > 0)
    throw new Error("owned background workload remains alive");
  await verifyNoJobs(ownership, worktree);

  const wipRef = await snapshotRef(worktree, rec.key);
  const dirty = await leftovers(worktree);
  if (wipRef === null && dirty.length > 0)
    throw new Error(
      `refusing ${abandon ? "--abandon" : "cleanup"}: ${worktree} has uncommitted work and the snapshot failed`,
    );
  if ((await ownedWorkload(worktree, shellPid)).length > 0)
    throw new Error("new workload appeared before cleanup; retry after it exits");

  let removed = false;
  if (workspace === "") {
    process.stderr.write(`WARN: no workspace_id on record ${rec.key}; skipping worktree remove\n`);
  } else {
    Object.assign(rec, {
      stage: "cleanup-incomplete",
      removal_intent: {
        version: 1,
        workspace_id: workspace,
        worktree,
        prepared_at: new Date().toISOString(),
        wip_ref: wipRef,
        leftover_files: dirty,
        workload: owned,
      } satisfies RemovalIntent,
    });
    await writeJson(path, rec);
    const force = abandon ? [] : ["--force"];
    removed = (
      await attempt(["herdr", "worktree", "remove", "--workspace", workspace, ...force], "inherit")
    ).ok;
  }
  if (removed) return finishRemovedWorktree(path, rec, wipRef, dirty, abandon);
  const branchDeleted = await deleteBranch(rec, removed, abandon);
  return { removed, branchDeleted, wipRef, dirty };
}

async function verifyPreparedRemoval(
  rec: IssueRecord,
  intent: RemovalIntent,
  ownership: CleanupOwnership,
): Promise<void> {
  if (intent.worktree !== rec.worktree || intent.workspace_id !== rec.workspace_id)
    throw new Error("removal intent does not match recorded workspace ownership");
  const agent = str(rec.agent);
  if (agent && (await inspectAgent(agent)))
    throw new Error(`agent ${agent} is still live after worktree removal`);
  if (!(await verifyOwnedExit(intent.workload)))
    throw new Error("owned background workload remains alive after worktree removal");
  await verifyNoJobs(ownership, rec.worktree);
  const listed = await runCommand(
    ["git", "-C", rec.checkout, "worktree", "list", "--porcelain", "-z"],
    { timeoutMs: 30_000 },
  );
  if (listed.exitCode !== 0 || listed.timedOut || listed.cancelled || listed.truncated)
    throw new Error("cannot verify removed worktree registration");
  const registered = listed.stdout
    .split("\0")
    .filter((line) => line.startsWith("worktree "))
    .map((line) => normalize(line.slice("worktree ".length)));
  if (registered.includes(normalize(rec.worktree)))
    throw new Error("worktree path is absent but remains registered");
}

/** Persist the destructive worktree-removal boundary before touching its branch. */
export async function finishRemovedWorktree(
  path: string,
  rec: IssueRecord,
  wipRef: string | null,
  dirty: string[],
  abandon: boolean,
): Promise<CleanupResult> {
  Object.assign(rec, {
    stage: "cleanup-incomplete",
    worktree_removed: true,
    branch_deleted: false,
    wip_ref: wipRef,
    leftover_files: dirty,
  });
  await writeJson(path, rec);
  return {
    removed: true,
    branchDeleted: await deleteBranch(rec, true, abandon),
    wipRef,
    dirty,
  };
}

async function finishAfterRemoval(
  rec: IssueRecord,
  ownership: CleanupOwnership | null,
  abandon: boolean,
): Promise<CleanupResult> {
  await verifyNoJobs(ownership, rec.worktree);
  return {
    removed: true,
    branchDeleted: await deleteBranch(rec, true, abandon),
    wipRef: typeof rec.wip_ref === "string" ? rec.wip_ref : null,
    dirty: Array.isArray(rec.leftover_files)
      ? rec.leftover_files.filter((entry): entry is string => typeof entry === "string")
      : [],
  };
}

async function verifyNoJobs(ownership: CleanupOwnership | null, worktree: string): Promise<void> {
  if (!ownership) return;
  await reconcileResourceJobs(ownership.root);
  if (activeJobs(ownership.root, worktree))
    throw new Error("managed job ownership remains; wait or reconcile before cleanup");
}

export async function deleteBranch(
  rec: IssueRecord,
  removed: boolean,
  abandon: boolean,
): Promise<boolean> {
  if (rec.branch_deleted === true) return true;
  if (!removed || abandon) return false;
  const ref = `refs/heads/${rec.branch}`;
  const present = await runCommand(
    ["git", "-C", rec.checkout, "show-ref", "--verify", "--quiet", ref],
    { timeoutMs: 30_000 },
  );
  if (present.timedOut || present.cancelled || present.truncated || present.exitCode > 1)
    throw new Error(`cannot verify local branch ${ref}`);
  if (present.exitCode === 1) return true;
  return (await attempt(["git", "-C", rec.checkout, "branch", "-D", rec.branch])).ok;
}

function parseArgs(argv: string[]): {
  stateDir: string;
  key: string;
  abandon: boolean;
} {
  const [stateDir, key, mode, ...extra] = argv;
  if (!stateDir || !key || extra.length > 0 || (mode !== undefined && mode !== "--abandon")) {
    throw new UsageError(USAGE);
  }
  return { stateDir, key, abandon: mode === "--abandon" };
}

async function main(): Promise<void> {
  const { stateDir, key, abandon } = parseArgs(process.argv.slice(2));
  const result = await withLaunchOwnership(stateDir, key, () =>
    finishIssue(stateDir, key, abandon),
  );
  process.stdout.write(`${JSON.stringify(result)}\n`);
  if (result.stage === "cleanup-incomplete") process.exitCode = 1;
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(`${err instanceof Error ? err.message : String(err)}\n`);
    process.exit(err instanceof UsageError ? 2 : 1);
  });
}
