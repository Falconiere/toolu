/** Strict finish records and durable adoption of legacy worktree resource bindings. */
import { existsSync, statSync } from "node:fs";
import { isAbsolute } from "node:path";
import {
  WorktreeJobsActiveError,
  acquireLease,
  fenceWorktree,
  readResourceState,
  releaseLease,
} from "../hooks/dist/epic-runtime.js";
import { resourceBinding } from "../hooks/dist/epic-runtime.js";
import { readJson, writeJson } from "./common.ts";

export type IssueRecord = Record<string, unknown> & {
  key: string;
  repo: string;
  branch: string;
  checkout: string;
  worktree: string;
  stage: string;
};

export type CleanupOwnership = { root: string; token: string };

export type RemovalIntent = {
  version: 1;
  workspace_id: string;
  worktree: string;
  prepared_at: string;
  wip_ref: string | null;
  leftover_files: string[];
  workload: { pid: number; group: number; started: string }[];
};

const STAGES = new Set([
  "starting",
  "running",
  "awaiting_merge",
  "replacing",
  "cleaning",
  "uncertain",
  "cleanup-incomplete",
  "merged",
  "abandoned",
]);

function isDirectory(path: string): boolean {
  return existsSync(path) && statSync(path).isDirectory();
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

export function removalIntent(rec: Record<string, unknown>): RemovalIntent | null {
  const value = rec.removal_intent;
  if (value === undefined) return null;
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error("invalid removal intent");
  const intent = value as Record<string, unknown>;
  const workload = intent.workload;
  if (
    intent.version !== 1 ||
    typeof intent.workspace_id !== "string" ||
    intent.workspace_id === "" ||
    typeof intent.worktree !== "string" ||
    !isAbsolute(intent.worktree) ||
    typeof intent.prepared_at !== "string" ||
    !Number.isFinite(Date.parse(intent.prepared_at)) ||
    (intent.wip_ref !== null && typeof intent.wip_ref !== "string") ||
    !Array.isArray(intent.leftover_files) ||
    !intent.leftover_files.every((entry) => typeof entry === "string") ||
    !Array.isArray(workload) ||
    !workload.every(
      (entry) =>
        typeof entry === "object" &&
        entry !== null &&
        !Array.isArray(entry) &&
        isPositiveInteger((entry as Record<string, unknown>).pid) &&
        isPositiveInteger((entry as Record<string, unknown>).group) &&
        typeof (entry as Record<string, unknown>).started === "string" &&
        (entry as Record<string, unknown>).started !== "",
    )
  )
    throw new Error("invalid removal intent");
  return intent as RemovalIntent;
}

export function readIssueRecord(path: string, key: string): IssueRecord {
  const value = readJson<unknown>(path, null);
  if (typeof value !== "object" || value === null || Array.isArray(value))
    throw new Error(`invalid or missing issue record ${path}`);
  const rec = value as Record<string, unknown>;
  for (const field of ["key", "repo", "branch", "checkout", "worktree", "stage"] as const) {
    if (typeof rec[field] !== "string" || rec[field] === "")
      throw new Error(`invalid issue record ${path}: ${field}`);
  }
  if (rec.key !== key) throw new Error(`invalid issue record ${path}: key mismatch`);
  if (!STAGES.has(String(rec.stage))) throw new Error(`invalid issue record ${path}: stage`);
  if (!isAbsolute(String(rec.checkout)) || !isAbsolute(String(rec.worktree)))
    throw new Error(`invalid issue record ${path}: paths`);
  const root = rec.resource_root;
  const token = rec.lease_token;
  if ((root === undefined) !== (token === undefined))
    throw new Error(`invalid issue record ${path}: partial resource ownership`);
  if (
    (root !== undefined && (typeof root !== "string" || !isAbsolute(root))) ||
    (token !== undefined && (typeof token !== "string" || token === ""))
  )
    throw new Error(`invalid issue record ${path}: resource ownership`);
  try {
    removalIntent(rec);
  } catch {
    throw new Error(`invalid issue record ${path}: removal intent`);
  }
  return rec as IssueRecord;
}

/** Resolve a modern token or atomically adopt a bound legacy worktree, then fence jobs. */
export async function fenceCleanupOwnership(
  path: string,
  stateDir: string,
  key: string,
  rec: IssueRecord,
): Promise<CleanupOwnership | null> {
  const binding = isDirectory(rec.worktree) ? resourceBinding(rec.worktree) : null;
  let root = typeof rec.resource_root === "string" ? rec.resource_root : undefined;
  let token = typeof rec.lease_token === "string" ? rec.lease_token : undefined;
  if (binding && (binding.key !== key || binding.stateDir !== stateDir))
    throw new Error("worktree resource binding does not match issue ownership");
  if (binding && root && binding.root !== root)
    throw new Error("recorded resource root differs from worktree binding");
  if (!root && binding) {
    root = binding.root;
    const matches = readResourceState(root).leases.filter(
      (lease) =>
        lease.type === "agent" &&
        lease.key === key &&
        lease.stateDir === stateDir &&
        (lease.worktree === undefined || lease.worktree === rec.worktree),
    );
    if (matches.length > 1) throw new Error("ambiguous legacy agent ownership");
    const lease =
      matches[0] ??
      (await acquireLease(root, {
        type: "agent",
        key,
        stateDir,
        worktree: rec.worktree,
      }));
    token = lease.token;
    Object.assign(rec, { resource_root: root, lease_token: token });
    await writeJson(path, rec);
  }
  if (!root || !token) return null;
  const recorded = readResourceState(root).leases.find((lease) => lease.token === token);
  if (
    recorded?.type !== "agent" ||
    recorded.key !== key ||
    recorded.stateDir !== stateDir ||
    (recorded.worktree !== undefined && recorded.worktree !== rec.worktree)
  )
    throw new Error("recorded agent resource ownership does not match issue");
  try {
    await fenceWorktree(root, token, rec.worktree);
  } catch (error) {
    if (!(error instanceof WorktreeJobsActiveError)) throw error;
  }
  return { root, token };
}

export async function releaseRecordedOwnership(rec: IssueRecord, stateDir: string): Promise<void> {
  if (typeof rec.resource_root !== "string" || typeof rec.lease_token !== "string") return;
  const lease = readResourceState(rec.resource_root).leases.find(
    (candidate) => candidate.token === rec.lease_token,
  );
  if (lease === undefined) return;
  if (
    lease.type !== "agent" ||
    lease.key !== rec.key ||
    lease.stateDir !== stateDir ||
    (lease.worktree !== undefined && lease.worktree !== rec.worktree)
  )
    throw new Error("recorded agent resource ownership does not match issue");
  await releaseLease(rec.resource_root, rec.lease_token);
}
