/** Machine admission over real isolated state and competing processes. */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { processGroupAlive, signalProcessGroup } from "../../process/process.ts";
import {
  acquireLease,
  acquireLock,
  cancelAgentMigration,
  coolResourceHost,
  fenceWorktree,
  migrateAgentLease,
  patchLease,
  prepareAgentMigration,
  readResourceState,
  reconcileResourceJobs,
  refreshPressure,
  releaseLease,
  writeJsonAtomic,
} from "../resources.ts";
import { activeJobs, runManagedJob } from "../jobs.ts";
import type { ResourceBinding } from "../binding.ts";
import { advancePressure } from "../pressure.ts";

test.concurrent("two epics share an atomic machine agent limit", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const results = await Promise.all(
    ["epic-a", "epic-b"].map((epic) =>
      run([process.execPath, join(import.meta.dir, "resource-worker.ts"), root, epic]),
    ),
  );
  expect(results.map((r) => r.exitCode).sort()).toEqual([0, 75]);
  const state = readResourceState(root);
  expect(state.leases).toHaveLength(1);
  const lease = state.leases[0];
  if (!lease) throw new Error("missing lease");
  // A launcher exiting does not establish that its agent exited.
  await expect(
    acquireLease(root, { type: "agent", key: "c", stateDir: "third", host: "claude" }),
  ).rejects.toThrow("capacity");
  await releaseLease(root, lease.token);
  expect(readResourceState(root).leases).toHaveLength(0);
});

test.concurrent("malformed state and invalid capacity fail closed", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 0 });
  await expect(
    acquireLease(root, { type: "agent", key: "a", stateDir: "a", host: "claude" }),
  ).rejects.toThrow("maxAgents");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1 });
  await writeJsonAtomic(join(root, "state.json"), { version: 999 });
  expect(() => readResourceState(root)).toThrow("resource state");
  await writeJsonAtomic(join(root, "state.json"), {
    version: 1,
    leases: [],
    cooldowns: { claude: { until: "later", reason: "busy" } },
  });
  expect(() => readResourceState(root)).toThrow("resource state");
});

test("host and epic limits count existing agents and lease tokens cannot release peers", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), {
    maxAgents: 3,
    maxJobs: 1,
    hosts: { claude: 1, codex: 2 },
  });
  const first = await acquireLease(root, {
    type: "agent",
    key: "first",
    stateDir: "epic-a",
    host: "claude",
  });
  await expect(
    acquireLease(root, { type: "agent", key: "second", stateDir: "epic-b", host: "claude" }),
  ).rejects.toThrow("claude capacity");
  const codex = await acquireLease(root, {
    type: "agent",
    key: "codex",
    stateDir: "epic-a",
    host: "codex",
    epicCap: 2,
  });
  await expect(
    acquireLease(root, {
      type: "agent",
      key: "third",
      stateDir: "epic-a",
      host: "codex",
      epicCap: 2,
    }),
  ).rejects.toThrow("epic capacity");
  await releaseLease(root, "not-a-token");
  expect(
    readResourceState(root)
      .leases.map((lease) => lease.token)
      .sort(),
  ).toEqual([first.token, codex.token].sort());
  await releaseLease(root, first.token);
  await releaseLease(root, codex.token);
});

test("agent migration preserves ownership until target admission succeeds", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), {
    maxAgents: 3,
    maxJobs: 1,
    hosts: { claude: 1, codex: 1 },
  });
  const source = await acquireLease(root, {
    type: "agent",
    key: "moving",
    stateDir: "epic-a",
    host: "claude",
    worktree: sb.project,
  });
  await patchLease(root, source.token, { stage: "running" });
  const occupied = await acquireLease(root, {
    type: "agent",
    key: "occupied",
    stateDir: "epic-b",
    host: "codex",
  });

  await expect(
    prepareAgentMigration(root, source.token, "codex", { hostCap: 1, epicCap: 2 }),
  ).rejects.toThrow("codex capacity");
  expect(
    readResourceState(root).leases.find((lease) => lease.token === source.token),
  ).toMatchObject({
    host: "claude",
    stage: "running",
  });
  await releaseLease(root, occupied.token);
  await coolResourceHost(root, "codex", Date.now() + 60_000, "provider limit");
  await expect(
    prepareAgentMigration(root, source.token, "codex", { hostCap: 1, epicCap: 2 }),
  ).rejects.toThrow("cooling down");
  await coolResourceHost(root, "codex", Date.now() - 1, "recovered");

  const job = await acquireLease(root, {
    type: "job",
    key: "old-host-work",
    stateDir: "epic-a",
    worktree: sb.project,
  });
  const reserved = await prepareAgentMigration(root, source.token, "codex", {
    hostCap: 1,
    epicCap: 2,
  });
  expect(reserved).toMatchObject({
    token: source.token,
    host: "claude",
    pendingHost: "codex",
    stage: "replacing",
  });
  await expect(
    acquireLease(root, { type: "agent", key: "racer", stateDir: "epic-c", host: "codex" }),
  ).rejects.toThrow("codex capacity");
  await expect(migrateAgentLease(root, source.token, "codex")).rejects.toThrow("active jobs");
  await releaseLease(root, job.token);

  const migrated = await migrateAgentLease(root, source.token, "codex");
  expect(migrated).toMatchObject({ token: source.token, host: "codex", stage: "replacing" });
  expect(migrated.pendingHost).toBeUndefined();
  expect(
    readResourceState(root).leases.filter((lease) => lease.token === source.token),
  ).toHaveLength(1);
  await prepareAgentMigration(root, source.token, "claude", { hostCap: 1, epicCap: 2 });
  expect(await cancelAgentMigration(root, source.token)).toMatchObject({
    token: source.token,
    host: "codex",
    stage: "running",
  });
  await releaseLease(root, source.token);
});

test("lock publication is exclusive, token-safe, and recovers a dead owner", async () => {
  using sb = createSandbox();
  const path = join(sb.root, "resource.lock");
  const first = await acquireLock(path);
  if (!first) throw new Error("first lock was not acquired");
  expect(await acquireLock(path)).toBeNull();

  await writeJsonAtomic(join(path, "owner.json"), { pid: process.pid, token: "replacement" });
  first.release();
  expect(existsSync(path)).toBe(true);
  rmSync(path, { recursive: true });

  mkdirSync(path);
  await writeJsonAtomic(join(path, "owner.json"), { pid: 2_147_483_647, token: "dead" });
  const recovered = await acquireLock(path);
  expect(recovered).not.toBeNull();
  recovered?.release();
  await expect(acquireLock(path, { timeoutMs: Number.NaN })).rejects.toThrow("timeoutMs");
});

test.concurrent("incident steal holds new work and recovery requires sustained headroom", () => {
  const sample = {
    at: 0,
    cpus: 8,
    load: 40.43,
    availableBytes: 8e9,
    totalBytes: 32e9,
    steal: 0.8595,
  };
  let pressure = advancePressure(undefined, sample);
  expect(pressure.held).toBe(false);
  pressure = advancePressure(pressure, { ...sample, at: 60_000 });
  expect(pressure.held).toBe(true);
  const recovered = { ...sample, load: 1, steal: 0, at: 90_000 };
  pressure = advancePressure(pressure, recovered);
  expect(pressure.held).toBe(true);
  pressure = advancePressure(pressure, { ...recovered, at: 210_000 });
  expect(pressure.held).toBe(false);
  expect(() => advancePressure(pressure, { ...sample, at: Number.NaN })).toThrow("pressure");
  expect(() => advancePressure(pressure, { ...sample, at: 1 })).toThrow("backwards");
});

test("recent pressure is reused and malformed persisted pressure fails closed", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const sample = {
    at: Date.now(),
    cpus: 8,
    load: 1,
    availableBytes: 24e9,
    totalBytes: 32e9,
    steal: 0,
  };
  const pressure = advancePressure(undefined, sample);
  await writeJsonAtomic(join(root, "state.json"), {
    version: 1,
    leases: [],
    cooldowns: {},
    pressure,
  });
  expect(await refreshPressure(root)).toEqual(pressure);
  await writeJsonAtomic(join(root, "state.json"), {
    version: 1,
    leases: [],
    cooldowns: {},
    pressure: { ...pressure, sample: { ...sample, load: "high" } },
  });
  expect(() => readResourceState(root)).toThrow("resource state");
});

function binding(root: string, worktree: string): ResourceBinding {
  return { version: 1, root, key: "issue-376", stateDir: join(root, "epic"), worktree };
}

async function managedLease(root: string) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const lease = readResourceState(root).leases.find((candidate) => candidate.type === "job");
    if (lease?.groupPid !== undefined) return lease;
    await Bun.sleep(20);
  }
  throw new Error("managed job did not publish its process group");
}

async function waitForMarker(path: string): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (existsSync(path)) return;
    await Bun.sleep(20);
  }
  throw new Error("managed workload did not start");
}

test("managed jobs retain capacity until redirected descendants exit", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const owned = binding(root, sb.project);
  const started = performance.now();
  const running = runManagedJob(["sh", "-c", "sleep 0.4 >/dev/null 2>&1 &"], owned);

  await Bun.sleep(100);
  expect(activeJobs(root, sb.project)).toBe(true);
  await expect(runManagedJob(["true"], owned)).rejects.toThrow("capacity");
  expect((await running).exitCode).toBe(0);
  expect(performance.now() - started).toBeGreaterThan(300);
  expect(activeJobs(root, sb.project)).toBe(false);
});

test("managed admission lets bounded CPU work finish and report real usage", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const script = [
    "const started = performance.now();",
    "let rounds = 0; let value = 1;",
    "while (performance.now() - started < 250) {",
    "  for (let i = 0; i < 1000; i += 1) value = Math.imul(value ^ i, 1664525) + 1013904223;",
    "  rounds += 1000;",
    "}",
    "const { userCPUTime, systemCPUTime } = process.resourceUsage();",
    "process.stdout.write(JSON.stringify({ rounds, value, userCPUTime, systemCPUTime }));",
  ].join("\n");

  const result = await runManagedJob([process.execPath, "-e", script], binding(root, sb.project));
  const usage = JSON.parse(result.stdout) as {
    rounds: number;
    value: number;
    userCPUTime: number;
    systemCPUTime: number;
  };
  expect(result).toMatchObject({ exitCode: 0, timedOut: false, cancelled: false });
  expect(usage.rounds).toBeGreaterThan(0);
  expect(Number.isInteger(usage.value)).toBe(true);
  expect(usage.userCPUTime + usage.systemCPUTime).toBeGreaterThan(0);
  expect(activeJobs(root, sb.project)).toBe(false);
});

test("a worktree fence blocks new jobs and retains agent ownership until old jobs exit", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const marker = join(sb.root, "must-not-start");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const agent = await acquireLease(root, {
    type: "agent",
    key: "issue-376",
    stateDir: join(root, "epic"),
    host: "codex",
    worktree: sb.project,
  });
  await patchLease(root, agent.token, { stage: "running" });
  const running = runManagedJob(
    [process.execPath, "-e", "await Bun.sleep(400)"],
    binding(root, sb.project),
  );
  await managedLease(root);

  await expect(releaseLease(root, agent.token)).rejects.toThrow("active jobs");
  await expect(fenceWorktree(root, agent.token, sb.project)).rejects.toThrow("active jobs");
  expect(readResourceState(root).leases.find((lease) => lease.token === agent.token)?.stage).toBe(
    "cleanup-incomplete",
  );
  expect((await running).exitCode).toBe(0);

  const fenced = await fenceWorktree(root, agent.token, sb.project);
  expect(fenced.stage).toBe("cleaning");
  await expect(
    runManagedJob(["sh", "-c", `echo started > '${marker}'`], binding(root, sb.project)),
  ).rejects.toThrow("worktree job admission blocked");
  expect(existsSync(marker)).toBe(false);
  await releaseLease(root, agent.token);
});

test("capacity refusal does not spawn a managed command", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const marker = join(sb.root, "started");
  const owned = binding(root, sb.project);
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const occupied = await acquireLease(root, {
    type: "job",
    key: "occupied",
    stateDir: "other",
  });

  await expect(runManagedJob(["sh", "-c", `echo started > '${marker}'`], owned)).rejects.toThrow(
    "capacity",
  );
  expect(existsSync(marker)).toBe(false);
  await releaseLease(root, occupied.token);
});

test("a normal owner signal cancels its managed process group", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const marker = join(sb.root, "started");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const runner = Bun.spawn(
    [process.execPath, join(import.meta.dir, "managed-job-worker.ts"), root, sb.project, marker],
    { stdout: "ignore", stderr: "ignore" },
  );
  const lease = await managedLease(root);
  const group = lease.groupPid ?? 0;
  await waitForMarker(marker);

  runner.kill("SIGTERM");
  await runner.exited;
  await Bun.sleep(100);

  expect(processGroupAlive(group)).toBe(false);
  const replacement = await acquireLease(root, {
    type: "job",
    key: "replacement",
    stateDir: "replacement",
  });
  await releaseLease(root, replacement.token);
});

test("a crashed owner keeps capacity reserved until its live group is gone", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const marker = join(sb.root, "started");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, maxJobs: 1 });
  const runner = Bun.spawn(
    [process.execPath, join(import.meta.dir, "managed-job-worker.ts"), root, sb.project, marker],
    { stdout: "ignore", stderr: "ignore" },
  );
  const lease = await managedLease(root);
  const group = lease.groupPid ?? 0;
  await waitForMarker(marker);

  runner.kill("SIGKILL");
  await runner.exited;
  expect(processGroupAlive(group)).toBe(true);
  await expect(
    acquireLease(root, { type: "job", key: "blocked", stateDir: "blocked" }),
  ).rejects.toThrow("capacity");
  expect(await reconcileResourceJobs(root)).toBe(0);

  signalProcessGroup(group, "SIGKILL");
  for (let attempt = 0; attempt < 20 && processGroupAlive(group); attempt += 1) {
    await Bun.sleep(50);
  }
  expect(await reconcileResourceJobs(root)).toBe(1);
  const replacement = await acquireLease(root, {
    type: "job",
    key: "replacement",
    stateDir: "replacement",
  });
  await releaseLease(root, replacement.token);
});
