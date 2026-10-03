/** Per-user machine capacity shared across host profiles and epic directories. */
import { randomUUID } from "node:crypto";
import { processGroupAlive } from "../process/process.ts";
import { processAlive } from "./lock.ts";
import { PRESSURE_SAMPLE_MS, advancePressure, sampleResources, type Pressure } from "./pressure.ts";
import {
  resourcePolicy,
  updateResources,
  type Lease,
  type LeaseRequest,
  type ResourceState,
} from "./resource-store.ts";

export { acquireLock, writeJsonAtomic, withResourceLock } from "./lock.ts";
export {
  cancelAgentMigration,
  fenceWorktree,
  migrateAgentLease,
  prepareAgentMigration,
  type AgentMigrationOptions,
  WorktreeJobsActiveError,
} from "./lease-transitions.ts";
export {
  readResourceState,
  resourceHome,
  resourcePolicy,
  updateResources,
  type Lease,
  type LeaseRequest,
  type ResourcePolicy,
  type ResourceState,
} from "./resource-store.ts";

function reconcileJobs(state: ResourceState): number {
  const before = state.leases.length;
  state.leases = state.leases.filter(
    (lease) =>
      lease.type !== "job" ||
      processAlive(lease.ownerPid) ||
      (lease.groupPid !== undefined && processGroupAlive(lease.groupPid)),
  );
  return before - state.leases.length;
}

/** Reclaim only jobs whose owner and recorded process group are both provably gone. */
export async function reconcileResourceJobs(root: string): Promise<number> {
  return updateResources(root, reconcileJobs);
}

function requireJobAdmission(state: ResourceState, req: LeaseRequest): void {
  if (req.type !== "job" || req.worktree === undefined) return;
  const agent = state.leases.find(
    (lease) => lease.type === "agent" && lease.worktree === req.worktree,
  );
  if (agent && agent.stage !== "starting" && agent.stage !== "running")
    throw new Error(`worktree job admission blocked by agent stage ${agent.stage}`);
}

function validateRequestCaps(req: LeaseRequest): void {
  if (req.hostCap !== undefined && (!Number.isSafeInteger(req.hostCap) || req.hostCap <= 0))
    throw new Error("invalid host capacity");
  if (req.epicCap !== undefined && (!Number.isSafeInteger(req.epicCap) || req.epicCap <= 0))
    throw new Error("invalid epic capacity");
}

/** Dead agent launchers never prove worker exit. Dead jobs may be recovered only
 * after both owner and owned process group are gone. */
export async function acquireLease(root: string, req: LeaseRequest): Promise<Lease> {
  validateRequestCaps(req);
  return updateResources(root, (state) => {
    const policy = resourcePolicy(root);
    reconcileJobs(state);
    if (!state.pressure || Date.now() - state.pressure.sample.at >= PRESSURE_SAMPLE_MS)
      state.pressure = advancePressure(state.pressure, sampleResources(state.pressure?.sample));
    if (state.pressure.held) throw new Error(`resource hold: ${state.pressure.reason}`);
    const live = state.leases.filter((lease) => lease.type === req.type);
    requireJobAdmission(state, req);
    if (live.some((lease) => lease.stateDir === req.stateDir && lease.key === req.key))
      throw new Error("existing ownership; reconcile before retry");
    const limit = req.type === "agent" ? policy.maxAgents : policy.maxJobs;
    if (live.length >= limit) throw new Error(`${req.type} capacity exhausted (${limit})`);
    if (
      req.type === "agent" &&
      req.epicCap !== undefined &&
      live.filter((lease) => lease.stateDir === req.stateDir).length >= req.epicCap
    )
      throw new Error("epic capacity exhausted");
    if (req.type === "agent" && req.host) {
      const hostLimit = Math.min(
        req.hostCap ?? policy.maxAgents,
        policy.hosts[req.host] ?? policy.maxAgents,
      );
      if (!Number.isSafeInteger(hostLimit) || hostLimit <= 0)
        throw new Error("invalid host capacity");
      if ((state.cooldowns[req.host]?.until ?? 0) > Date.now())
        throw new Error(`${req.host} is cooling down`);
      if (
        live.filter((lease) => lease.host === req.host || lease.pendingHost === req.host).length >=
        hostLimit
      )
        throw new Error(`${req.host} capacity exhausted`);
    }
    const now = new Date().toISOString();
    const lease: Lease = {
      token: randomUUID(),
      type: req.type,
      key: req.key,
      stateDir: req.stateDir,
      ownerPid: process.pid,
      stage: "starting",
      createdAt: now,
      heartbeatAt: now,
    };
    if (req.host !== undefined) lease.host = req.host;
    if (req.worktree !== undefined) lease.worktree = req.worktree;
    state.leases.push(lease);
    return lease;
  });
}

export async function patchLease(
  root: string,
  token: string,
  patch: Partial<
    Pick<Lease, "groupPid" | "worktree" | "pane" | "session" | "stage" | "heartbeatAt">
  >,
): Promise<void> {
  if (
    patch.groupPid !== undefined &&
    (!Number.isSafeInteger(patch.groupPid) || patch.groupPid <= 0)
  )
    throw new Error("invalid resource lease patch");
  await updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (!lease) throw new Error("resource lease lost");
    Object.assign(lease, patch);
  });
}

/** Caller must verify workload exit. Agent release also proves no managed job remains. */
export async function releaseLease(root: string, token: string): Promise<void> {
  await updateResources(root, (state) => {
    reconcileJobs(state);
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (
      lease?.type === "agent" &&
      lease.worktree !== undefined &&
      state.leases.some(
        (candidate) => candidate.type === "job" && candidate.worktree === lease.worktree,
      )
    )
      throw new Error("active jobs prevent agent lease release");
    state.leases = state.leases.filter((candidate) => candidate.token !== token);
  });
}

export async function coolResourceHost(
  root: string,
  host: string,
  until: number,
  reason: string,
): Promise<void> {
  if (!host || !Number.isFinite(until) || !reason) throw new Error("invalid resource cooldown");
  await updateResources(root, (state) => {
    state.cooldowns[host] = { until, reason };
  });
}

export async function refreshPressure(root: string): Promise<Pressure> {
  return updateResources(root, (state) => {
    if (!state.pressure || Date.now() - state.pressure.sample.at >= PRESSURE_SAMPLE_MS)
      state.pressure = advancePressure(state.pressure, sampleResources(state.pressure?.sample));
    return state.pressure;
  });
}
