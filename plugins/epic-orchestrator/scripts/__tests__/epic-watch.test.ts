/** Watcher event rules over status files the real report.ts writes. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { coolHost, issueEvents, limitEvents, limitLine } from "../epic-watch.ts";
import { coolingHosts } from "../route.ts";

const REPORT = join(import.meta.dir, "..", "report.ts");
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
