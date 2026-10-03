/** Watcher event rules over status files the real report.ts writes. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import {
  acknowledgeStall,
  ageMinutes,
  coolHost,
  initialWatchRuntime,
  issueEvents,
  limitEvents,
  limitLine,
  parseWatchArgs,
  readWatchRuntime,
  takeWatchLock,
} from "../epic-watch.ts";
import { coolingHosts } from "../route.ts";

const REPORT = join(import.meta.dir, "..", "report.ts");
const WATCH = join(import.meta.dir, "..", "epic-watch.ts");
const REC = {
  ref: "Falconiere/comemory#255",
  agent: "comemory-255",
  stage: "running",
  last_launch: "2026-09-24T10:00:00Z",
};

type Seen = Record<string, Record<string, unknown>>;

/** Report `args` into `<dir>/status/k.json` through the real CLI and read it back. */
async function reported(dir: string, ...args: string[]): Promise<Record<string, unknown>> {
  const file = join(dir, "status", "k.json");
  const res = await run(["bun", REPORT, file, ...args]);
  expect(res.exitCode).toBe(0);
  return JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
}

test.concurrent("ready is reported once", async () => {
  using sb = createSandbox();
  const seen: Seen = {};
  const st = await reported(sb.root, "ready", "--pr", "301", "--note", "n");
  const first = issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, seen);
  expect(first.map((e) => e.type)).toEqual(["ready"]);
  expect(issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, seen)).toEqual([]);
});

test.concurrent("blocked once per episode", async () => {
  using sb = createSandbox();
  const seen: Seen = {};
  const st = await reported(sb.root, "execution");
  const agents = { "comemory-255": "blocked" };
  expect(issueEvents("comemory-255", REC, st, agents, seen).map((e) => e.type)).toEqual([
    "blocked",
  ]);
  expect(issueEvents("comemory-255", REC, st, agents, seen)).toEqual([]);
  issueEvents("comemory-255", REC, st, { "comemory-255": "working" }, seen);
  expect(issueEvents("comemory-255", REC, st, agents, seen).map((e) => e.type)).toEqual([
    "blocked",
  ]);
});

test.concurrent("gone only while running and herdr reachable", async () => {
  using sb = createSandbox();
  const st = await reported(sb.root, "plan");
  expect(issueEvents("comemory-255", REC, st, {}, {}).map((e) => e.type)).toEqual(["gone"]);
  expect(issueEvents("comemory-255", REC, st, { __error__: "socket" }, {})).toEqual([]);
});

test.concurrent("idle agent with old status is stalled but babysit is patient", async () => {
  using sb = createSandbox();
  const st = { ...(await reported(sb.root, "plan")), updated_at: "2026-09-24T08:00:00Z" };
  const types = issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, {}).map(
    (e) => e.type,
  );
  expect(types).toEqual(["stalled"]);
  const babysitStatus = await reported(sb.root, "babysit");
  const freshBabysit = { ...st, phase: "babysit", updated_at: babysitStatus.updated_at };
  expect(issueEvents("comemory-255", REC, freshBabysit, { "comemory-255": "idle" }, {})).toEqual(
    [],
  );
});

test.concurrent("stale working and idle agents repeat on a bounded persisted deadline", () => {
  const now = Date.parse("2026-10-03T12:00:00Z");
  const status = { phase: "execution", updated_at: "2026-10-03T10:00:00Z" };
  const seen: Seen = {};
  const first = issueEvents("k", REC, status, { "comemory-255": "working" }, seen, now, 10);
  expect(first.filter((event) => event.type === "stalled")).toEqual([
    expect.objectContaining({ occurrence: 1, minutes: 120 }),
  ]);
  expect(
    issueEvents("k", REC, status, { "comemory-255": "idle" }, seen, now + 9 * 60_000, 10),
  ).toEqual([]);
  const repeated = issueEvents(
    "k",
    REC,
    status,
    { "comemory-255": "idle" },
    seen,
    now + 10 * 60_000,
    10,
  );
  expect(repeated).toEqual([expect.objectContaining({ type: "stalled", occurrence: 2 })]);

  const acknowledged = { ...status, updated_at: new Date(now + 11 * 60_000).toISOString() };
  expect(
    issueEvents("k", REC, acknowledged, { "comemory-255": "working" }, seen, now + 11 * 60_000),
  ).toEqual([]);
  expect(seen.k?.stall).toBeUndefined();
});

test.concurrent("stall acknowledgement persists and defers the next bounded escalation", () => {
  const now = Date.parse("2026-10-03T12:00:00Z");
  const status = { phase: "execution", updated_at: "2026-10-03T10:00:00Z" };
  const seen: Seen = {};
  issueEvents("k", REC, status, { "comemory-255": "working" }, seen, now, 10);
  const acknowledged = acknowledgeStall(seen, "k", now + 60_000, 10);
  expect(acknowledged?.acknowledgedAt).toBe("2026-10-03T12:01:00.000Z");
  expect(
    issueEvents("k", REC, status, { "comemory-255": "working" }, seen, now + 10 * 60_000, 10),
  ).toEqual([]);
  const repeated = issueEvents(
    "k",
    REC,
    status,
    { "comemory-255": "working" },
    seen,
    now + 11 * 60_000,
    10,
  );
  expect(repeated).toEqual([
    expect.objectContaining({ occurrence: 2, acknowledged_at: "2026-10-03T12:01:00.000Z" }),
  ]);
});

test.concurrent("parked ready and needs-human workers do not raise stall events", () => {
  const now = Date.parse("2026-10-03T12:00:00Z");
  for (const phase of ["ready", "needs-human"]) {
    const events = issueEvents(
      phase,
      REC,
      { phase, updated_at: "2026-10-03T01:00:00Z" },
      { "comemory-255": "idle" },
      {},
      now,
    );
    expect(events.some((event) => event.type === "stalled")).toBe(false);
  }
});

test.concurrent("ageMinutes respects explicit timezone offsets and treats unzoned stamps as UTC", () => {
  const now = Date.parse("2026-10-03T12:00:00Z");
  expect(ageMinutes("2026-10-03T08:00:00-03:00", now)).toBe(60);
  expect(ageMinutes("2026-10-03T11:00:00", now)).toBe(60);
  expect(ageMinutes("invalid", now)).toBe(0);
});

test.concurrent("watch timing arguments reject nonfinite, negative, and zero pacing", () => {
  const invalid = [
    ["--interval", "0"],
    ["--interval", "NaN"],
    ["--max-wait", "-1"],
    ["--recheck", "Infinity"],
    ["--checkpoint", "-1"],
  ];
  for (const args of invalid) {
    expect(() => parseWatchArgs(["--state-dir", "/tmp/state", ...args])).toThrow(/finite number/);
  }
  expect(parseWatchArgs(["--state-dir", "/tmp/state", "--checkpoint", "0"]).checkpointMin).toBe(0);
});

test.concurrent("watch CLI rejects invalid timing before acquiring ownership or polling herdr", async () => {
  using sb = createSandbox();
  const result = await run(["bun", WATCH, "--state-dir", sb.root, "--interval", "NaN"]);
  expect(result.exitCode).toBe(1);
  expect(result.stderr).toContain("--interval must be a positive finite number");
  expect(existsSync(join(sb.root, "watch.lock"))).toBe(false);
  expect(existsSync(join(sb.root, "watch-state.json"))).toBe(false);
});

test.concurrent("watch CLI acknowledges a persisted stall without polling herdr", async () => {
  using sb = createSandbox();
  writeFileSync(
    join(sb.root, "watch-seen.json"),
    JSON.stringify({
      k: {
        stall: {
          stamp: "2026-10-03T10:00:00Z",
          nextAlertAt: 0,
          alerts: 1,
        },
      },
    }),
  );
  const result = await run(["bun", WATCH, "--state-dir", sb.root, "--ack", "k"]);
  expect(result.exitCode).toBe(0);
  expect(JSON.parse(result.stdout)).toEqual(
    expect.objectContaining({
      acknowledged: "k",
      stall: expect.objectContaining({ alerts: 1 }),
    }),
  );
  const seen = JSON.parse(readFileSync(join(sb.root, "watch-seen.json"), "utf8")) as Seen;
  expect(seen.k?.stall).toEqual(expect.objectContaining({ acknowledgedAt: expect.any(String) }));
});

test.concurrent("watch runtime reloads persisted checkpoint, budget, and dependency deadlines", () => {
  using sb = createSandbox();
  const now = Date.parse("2026-10-03T12:00:00Z");
  const initial = initialWatchRuntime(now);
  expect(initial.nextCheckpointAt).toBe(now);
  const path = sb.write("watch-state.json", {
    ...initial,
    nextCheckpointAt: now + 60_000,
    checkpointQueue: ["k"],
    nextBudgetAt: now + 120_000,
    herdrFailures: 3,
    herdrRetryAt: now + 30_000,
  });
  expect(readWatchRuntime(path, now)).toEqual(
    expect.objectContaining({
      nextCheckpointAt: now + 60_000,
      checkpointQueue: ["k"],
      nextBudgetAt: now + 120_000,
      herdrFailures: 3,
      herdrRetryAt: now + 30_000,
    }),
  );
  const bad = sb.write("bad-watch-state.json", { ...initial, nextBudgetAt: Number.NaN });
  expect(() => readWatchRuntime(bad, now)).toThrow("invalid watcher state");
});

test.concurrent("watch lock is exclusive and a stale release token cannot remove a new owner", async () => {
  using sb = createSandbox();
  const first = await takeWatchLock(sb.root);
  expect(first).not.toBeNull();
  expect(await takeWatchLock(sb.root)).toBeNull();
  first?.release();
  const next = await takeWatchLock(sb.root);
  expect(next).not.toBeNull();
  first?.release();
  expect(await takeWatchLock(sb.root)).toBeNull();
  next?.release();
});

test.concurrent("limitLine finds the newest limit line in a pane tail", () => {
  const tail = [
    "  ✓ 12 tests passed",
    "  ■ You've hit your usage limit. Try again at 6:40 PM.",
    "",
    "  › Ask Codex to do anything",
  ].join("\n");
  expect(limitLine(tail)).toBe("■ You've hit your usage limit. Try again at 6:40 PM.");
  expect(limitLine("  ✓ all green\n  › waiting")).toBeNull();
});

test.concurrent("coolHost writes a cooldown that routing honors", async () => {
  using sb = createSandbox();
  const now = Date.parse("2026-09-26T12:00:00Z");
  await coolHost(sb.root, "cursor-agent", "usage limit", now);
  const hosts = JSON.parse(readFileSync(join(sb.root, "hosts.json"), "utf8")) as Record<
    string,
    { until: string; reason: string }
  >;
  expect(hosts.cursor?.reason).toBe("usage limit");
  expect([...coolingHosts(hosts, now + 30 * 60_000)]).toEqual(["cursor"]);
  expect([...coolingHosts(hosts, now + 61 * 60_000)]).toEqual([]);
});

test.concurrent("coolHost can publish the same cooldown to shared machine state", async () => {
  using sb = createSandbox();
  const now = Date.parse("2026-09-26T12:00:00Z");
  const resources = join(sb.root, "resources");
  expect(await coolHost(sb.root, "codex", "usage limit", now, resources)).toBe(true);
  const shared = JSON.parse(readFileSync(join(resources, "state.json"), "utf8")) as {
    cooldowns: Record<string, { until: number; reason: string }>;
  };
  expect(shared.cooldowns.codex).toEqual({
    until: now + 60 * 60_000,
    reason: "usage limit",
  });
});

test.concurrent("a worker's rate-limited report cools its host and raises host-limited once", async () => {
  using sb = createSandbox();
  const st = await reported(sb.root, "failed", "--note", "rate-limited: codex usage limit");
  const active = { k: { ...REC, kind: "codex" } };
  const seen = {};
  const agents = { "comemory-255": "working" };
  const first = await limitEvents(sb.root, active, { k: st }, agents, seen);
  expect(first).toEqual([
    {
      key: "k",
      ref: REC.ref,
      type: "host-limited",
      host: "codex",
      cooldown: true,
      note: "rate-limited: codex usage limit",
    },
  ]);
  const hosts = JSON.parse(readFileSync(join(sb.root, "hosts.json"), "utf8")) as Record<
    string,
    unknown
  >;
  expect(Object.keys(hosts)).toEqual(["codex"]);
  expect(await limitEvents(sb.root, active, { k: st }, agents, seen)).toEqual([]);
});

test.concurrent("a record naming an unknown host is still reported, without a cooldown or a crash", async () => {
  using sb = createSandbox();
  const st = await reported(sb.root, "failed", "--note", "rate-limited: quota exceeded");
  const events = await limitEvents(sb.root, { k: { ...REC, kind: "gemini" } }, { k: st }, {}, {});
  expect(events).toEqual([
    {
      key: "k",
      ref: REC.ref,
      type: "host-limited",
      host: "gemini",
      cooldown: false,
      note: "rate-limited: quota exceeded",
    },
  ]);
  expect(existsSync(join(sb.root, "hosts.json"))).toBe(false);
});
