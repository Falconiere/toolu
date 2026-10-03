/** Durable launch ownership is reserved before external mutations. */
import { join } from "node:path";
import {
  acquireLease,
  acquireLock,
  patchLease,
  readResourceState,
  resourceHome,
  reconcileResourceJobs,
  fenceWorktree,
  WorktreeJobsActiveError,
  prepareAgentMigration,
  migrateAgentLease,
  type Lease,
} from "../hooks/dist/epic-runtime.js";
import { writeJson, failureDetails } from "./common.ts";
import type { HostKind } from "./hosts.ts";
import { hostKind } from "./hosts.ts";
import { activeJobs } from "../hooks/dist/epic-runtime.js";
import { shutdownAgent } from "./lifecycle.ts";
import { loadHostPool } from "./launch-route.ts";

export async function launchReservation(
  state: string,
  key: string,
  host: HostKind,
  maxParallel: number,
  record: Record<string, unknown>,
  replace = false,
): Promise<{ root: string; lease: Lease }> {
  const root = typeof record.resource_root === "string" ? record.resource_root : resourceHome();
  const pool = loadHostPool(join(state, "pool.json"), [{ kind: host, cap: maxParallel }]);
  const selected = pool.find((h) => h.kind === host);
  if (!selected) throw new Error(`${host} is no longer in the host pool; reroute first`);
  const saved = readResourceState(root).leases.find(
    (l) => l.type === "agent" && l.stateDir === state && l.key === key,
  );
  if (saved) {
    if (record.lease_token !== saved.token)
      throw new Error("existing launch ownership differs; finish/reconcile it before retry");
    if (saved.host === host && !replace) return { root, lease: saved };
    if (
      !replace ||
      !saved.host ||
      typeof record.pane_id !== "string" ||
      typeof record.worktree !== "string"
    )
      throw new Error("replacement requires verified ownership and explicit --replace");
    const prepared = await prepareAgentMigration(root, saved.token, host, {
      hostCap: selected.cap,
      epicCap: maxParallel,
    });
    try {
      await persistAttempt(state, key, record, root, prepared, "replacing");
      try {
        await fenceWorktree(root, saved.token, record.worktree);
      } catch (error) {
        // Explicit replacement cancels existing jobs; the persisted fence blocks new ones.
        if (!(error instanceof WorktreeJobsActiveError)) throw error;
      }
      const stopped = await shutdownAgent(key, {
        kind: hostKind(saved.host),
        pane: record.pane_id,
        cwd: record.worktree,
      });
      await reconcileResourceJobs(root);
      if (stopped !== "exited" || activeJobs(root, record.worktree))
        throw new Error("old host workload remains; replacement refused");
      await persistAttempt(state, key, record, root, saved, "replacing");
      const lease = await migrateAgentLease(root, saved.token, host);
      return { root, lease };
    } catch (error) {
      Object.assign(record, failureDetails(error));
      await persistAttempt(state, key, record, root, saved, "uncertain");
      throw error;
    }
  }
  const lease = await acquireLease(root, {
    type: "agent",
    key,
    stateDir: state,
    host,
    hostCap: selected.cap,
    epicCap: maxParallel,
  });
  return { root, lease };
}

/** Serialize attempts for one issue, including prompt submission/reconciliation. */
export async function withLaunchOwnership<T>(
  state: string,
  key: string,
  work: () => Promise<T>,
): Promise<T> {
  const lock = await acquireLock(join(state, "launch-locks", key));
  if (!lock) throw new Error(`launch already in progress for ${key}`);
  try {
    return await work();
  } finally {
    lock.release();
  }
}

export async function persistAttempt(
  state: string,
  key: string,
  record: Record<string, unknown>,
  root: string,
  lease: Lease,
  stage: string,
): Promise<void> {
  Object.assign(record, {
    resource_root: root,
    lease_token: lease.token,
    attempt_id: lease.token,
    stage,
  });
  await writeJson(join(state, "issues", `${key}.json`), record);
  await patchLease(root, lease.token, {
    stage,
    ...(typeof record.worktree === "string" ? { worktree: record.worktree } : {}),
    ...(typeof record.pane_id === "string" ? { pane: record.pane_id } : {}),
    ...(typeof record.session_id === "string" ? { session: record.session_id } : {}),
  });
}
