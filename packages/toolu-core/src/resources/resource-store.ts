/** Strict persisted resource state and its single machine-wide mutation lock. */
import { homedir } from "node:os";
import { join } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import { readJsonFile, withResourceLock, writeJsonAtomic } from "./lock.ts";
import {
  PRESSURE_SAMPLE_MS,
  advancePressure,
  isPressure,
  sampleResources,
  type Pressure,
} from "./pressure.ts";

export type Lease = {
  token: string;
  type: "agent" | "job";
  key: string;
  stateDir: string;
  host?: string;
  pendingHost?: string;
  ownerPid: number;
  groupPid?: number;
  worktree?: string;
  pane?: string;
  session?: string;
  stage: string;
  createdAt: string;
  heartbeatAt: string;
};
export type ResourceState = {
  version: 1;
  leases: Lease[];
  cooldowns: Record<string, { until: number; reason: string }>;
  pressure?: Pressure;
};
export type ResourcePolicy = {
  maxAgents: number;
  maxJobs: number;
  hosts: Record<string, number>;
  pressure: boolean;
};
export type LeaseRequest = Pick<Lease, "type" | "key" | "stateDir"> & {
  host?: string;
  hostCap?: number;
  epicCap?: number;
  worktree?: string;
};

export function resourceHome(env: NodeJS.ProcessEnv = process.env): string {
  return env.TOOLU_RESOURCE_HOME ?? join(homedir(), ".local", "state", "toolu", "resources");
}

export function resourcePolicy(root: string): ResourcePolicy {
  const p = readJsonFile(join(root, "policy.json"), {});
  if (!isJsonObject(p)) throw new Error("invalid resource policy");
  const hosts: Record<string, number> = {};
  if (p.hosts !== undefined) {
    if (!isJsonObject(p.hosts)) throw new Error("invalid resource hosts");
    for (const [host, cap] of Object.entries(p.hosts)) {
      if (typeof cap !== "number") throw new Error(`invalid resource capacity ${host}`);
      hosts[host] = cap;
    }
  }
  const maxAgents = p.maxAgents ?? 3;
  const maxJobs = p.maxJobs ?? 1;
  if (typeof maxAgents !== "number" || typeof maxJobs !== "number")
    throw new Error("invalid resource capacity");
  const pressure = p.pressure ?? true;
  if (typeof pressure !== "boolean") throw new Error("invalid resource pressure policy");
  const policy = { maxAgents, maxJobs, hosts, pressure };
  for (const [key, value] of Object.entries({
    maxAgents: policy.maxAgents,
    maxJobs: policy.maxJobs,
    ...policy.hosts,
  })) {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new Error(`invalid resource capacity ${key}`);
  }
  return policy;
}

/** Refresh a stale pressure sample in place; callers hold the resource lock. */
export function freshPressure(state: ResourceState): Pressure {
  if (!state.pressure || Date.now() - state.pressure.sample.at >= PRESSURE_SAMPLE_MS)
    state.pressure = advancePressure(state.pressure, sampleResources(state.pressure?.sample));
  return state.pressure;
}

/** Refuse new work during a sustained pressure hold unless policy disables pressure. */
export function admitPressure(state: ResourceState, policy: ResourcePolicy): void {
  if (!policy.pressure) return;
  const pressure = freshPressure(state);
  if (pressure.held) throw new Error(`resource hold: ${pressure.reason}`);
}

function optionalString(value: unknown): boolean {
  return value === undefined || typeof value === "string";
}

function isLease(lease: unknown): lease is Lease {
  return (
    isJsonObject(lease) &&
    typeof lease.token === "string" &&
    lease.token.length > 0 &&
    (lease.type === "agent" || lease.type === "job") &&
    typeof lease.key === "string" &&
    typeof lease.stateDir === "string" &&
    optionalString(lease.host) &&
    optionalString(lease.pendingHost) &&
    typeof lease.ownerPid === "number" &&
    Number.isSafeInteger(lease.ownerPid) &&
    lease.ownerPid > 0 &&
    (lease.groupPid === undefined ||
      (typeof lease.groupPid === "number" &&
        Number.isSafeInteger(lease.groupPid) &&
        lease.groupPid > 0)) &&
    optionalString(lease.worktree) &&
    optionalString(lease.pane) &&
    optionalString(lease.session) &&
    typeof lease.stage === "string" &&
    typeof lease.createdAt === "string" &&
    typeof lease.heartbeatAt === "string"
  );
}

function isResourceState(state: unknown): state is ResourceState {
  return (
    isJsonObject(state) &&
    state.version === 1 &&
    Array.isArray(state.leases) &&
    isJsonObject(state.cooldowns) &&
    state.leases.every(isLease) &&
    Object.values(state.cooldowns).every(
      (value) =>
        isJsonObject(value) &&
        typeof value.until === "number" &&
        Number.isFinite(value.until) &&
        typeof value.reason === "string",
    ) &&
    (state.pressure === undefined || isPressure(state.pressure))
  );
}

export function readResourceState(root: string): ResourceState {
  const state = readJsonFile(join(root, "state.json"), {
    version: 1,
    leases: [],
    cooldowns: {},
  });
  if (!isResourceState(state))
    throw new Error("invalid resource state; reconcile ownership before launching");
  return state;
}

export async function updateResources<T>(
  root: string,
  fn: (state: ResourceState) => T,
): Promise<T> {
  return withResourceLock(root, () => {
    const state = readResourceState(root);
    try {
      return fn(state);
    } finally {
      writeJsonAtomic(join(root, "state.json"), state);
    }
  });
}
