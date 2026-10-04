/** Atomic ownership transitions that must never release capacity between states. */
import { processGroupAlive } from "../process/process.ts";
import { processAlive } from "./lock.ts";
import {
  admitPressure,
  resourcePolicy,
  updateResources,
  type Lease,
  type ResourceState,
} from "./resource-store.ts";

export type AgentMigrationOptions = { hostCap: number; epicCap?: number };

export class WorktreeJobsActiveError extends Error {
  constructor() {
    super("active jobs prevent worktree cleanup");
    this.name = "WorktreeJobsActiveError";
  }
}

function reconcileJobs(state: ResourceState): void {
  state.leases = state.leases.filter(
    (lease) =>
      lease.type !== "job" ||
      processAlive(lease.ownerPid) ||
      (lease.groupPid !== undefined && processGroupAlive(lease.groupPid)),
  );
}

function activeWorktreeJob(state: ResourceState, worktree: string): boolean {
  return state.leases.some((lease) => lease.type === "job" && lease.worktree === worktree);
}

function validateMigration(targetHost: string, options: AgentMigrationOptions): void {
  if (!targetHost) throw new Error("invalid target host");
  if (!Number.isSafeInteger(options.hostCap) || options.hostCap <= 0)
    throw new Error("invalid host capacity");
  if (
    options.epicCap !== undefined &&
    (!Number.isSafeInteger(options.epicCap) || options.epicCap <= 0)
  )
    throw new Error("invalid epic capacity");
}

function reserveTarget(
  root: string,
  state: ResourceState,
  lease: Lease,
  targetHost: string,
  options: AgentMigrationOptions,
): void {
  const policy = resourcePolicy(root);
  admitPressure(state, policy);
  const others = state.leases.filter(
    (candidate) => candidate.type === "agent" && candidate.token !== lease.token,
  );
  if (others.length >= policy.maxAgents)
    throw new Error(`agent capacity exhausted (${policy.maxAgents})`);
  if (
    options.epicCap !== undefined &&
    others.filter((candidate) => candidate.stateDir === lease.stateDir).length >= options.epicCap
  )
    throw new Error("epic capacity exhausted");
  const hostLimit = Math.min(options.hostCap, policy.hosts[targetHost] ?? policy.maxAgents);
  if ((state.cooldowns[targetHost]?.until ?? 0) > Date.now())
    throw new Error(`${targetHost} is cooling down`);
  if (
    others.filter(
      (candidate) => candidate.host === targetHost || candidate.pendingHost === targetHost,
    ).length >= hostLimit
  )
    throw new Error(`${targetHost} capacity exhausted`);
}

/** Reserve target capacity on the source token before any source shutdown mutation. */
export async function prepareAgentMigration(
  root: string,
  token: string,
  targetHost: string,
  options: AgentMigrationOptions,
): Promise<Lease> {
  validateMigration(targetHost, options);
  return updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (lease?.type !== "agent") throw new Error("agent resource lease lost");
    if (lease.pendingHost === targetHost) {
      lease.stage = "replacing";
      lease.heartbeatAt = new Date().toISOString();
      return { ...lease };
    }
    if (lease.pendingHost !== undefined) throw new Error("agent migration already reserved");
    reserveTarget(root, state, lease, targetHost, options);
    lease.pendingHost = targetHost;
    lease.stage = "replacing";
    lease.heartbeatAt = new Date().toISOString();
    return { ...lease };
  });
}

/** Consume a previously reserved target only after old jobs and workload have exited. */
export async function migrateAgentLease(
  root: string,
  token: string,
  targetHost: string,
): Promise<Lease> {
  return updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (lease?.type !== "agent") throw new Error("agent resource lease lost");
    if (lease.stage !== "replacing" || lease.pendingHost !== targetHost)
      throw new Error("agent target capacity is not reserved");
    reconcileJobs(state);
    if (lease.worktree && activeWorktreeJob(state, lease.worktree))
      throw new Error("active jobs prevent agent migration");
    lease.host = targetHost;
    delete lease.pendingHost;
    lease.heartbeatAt = new Date().toISOString();
    return { ...lease };
  });
}

/** Cancel an unused target reservation after the source is verified healthy. */
export async function cancelAgentMigration(root: string, token: string): Promise<Lease> {
  return updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === token);
    if (lease?.type !== "agent") throw new Error("agent resource lease lost");
    if (lease.pendingHost === undefined) throw new Error("agent migration is not reserved");
    delete lease.pendingHost;
    lease.stage = "running";
    lease.heartbeatAt = new Date().toISOString();
    return { ...lease };
  });
}

/** Persist a cleanup fence before shutdown; new jobs then fail admission. */
export async function fenceWorktree(
  root: string,
  agentToken: string,
  worktree: string,
): Promise<Lease> {
  if (!worktree) throw new Error("invalid fenced worktree");
  return updateResources(root, (state) => {
    const lease = state.leases.find((candidate) => candidate.token === agentToken);
    if (lease?.type !== "agent") throw new Error("agent resource lease lost");
    if (lease.worktree !== undefined && lease.worktree !== worktree)
      throw new Error("agent lease worktree does not match cleanup target");
    reconcileJobs(state);
    lease.worktree = worktree;
    lease.stage = "cleaning";
    lease.heartbeatAt = new Date().toISOString();
    if (activeWorktreeJob(state, worktree)) {
      lease.stage = "cleanup-incomplete";
      throw new WorktreeJobsActiveError();
    }
    return { ...lease };
  });
}
