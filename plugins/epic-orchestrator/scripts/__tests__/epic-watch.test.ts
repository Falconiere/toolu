/** Watcher event rules plus the real report.sh writing the status files it reads. */

import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { issueEvents } from "../epic-watch.ts";

const REPORT = join(import.meta.dir, "..", "report.sh");
const REC = {
  ref: "Falconiere/comemory#255",
  agent: "comemory-255",
  stage: "running",
  last_launch: "2026-09-24T10:00:00Z",
};

async function report(statusFile: string, ...args: string[]) {
  const proc = Bun.spawn(["bash", REPORT, statusFile, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, code] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  return { stdout, stderr, code };
}

describe("ReportTest", () => {
  test("history and pr persist", async () => {
    const dir = mkdtempSync(join(tmpdir(), "epic-watch-"));
    const f = join(dir, "status", "k.json");
    expect((await report(f, "spec")).code).toBe(0);
    expect((await report(f, "pr-open", "--pr", "301")).code).toBe(0);
    expect((await report(f, "babysit")).code).toBe(0);
    const data = JSON.parse(readFileSync(f, "utf8")) as {
      phase: string;
      pr: number;
      history: { phase: string }[];
    };
    expect(data.phase).toBe("babysit");
    expect(data.pr).toBe(301);
    expect(data.history.map((h) => h.phase)).toEqual(["spec", "pr-open", "babysit"]);
  });

  test("rejects unknown phase and bad pr", async () => {
    const dir = mkdtempSync(join(tmpdir(), "epic-watch-"));
    const f = join(dir, "k.json");
    expect((await report(f, "merged")).code).toBe(2);
    expect((await report(f, "ready", "--pr", "abc")).code).toBe(2);
    expect(() => readFileSync(f)).toThrow();
  });
});

describe("EventsTest", () => {
  async function status(phase: string) {
    const dir = mkdtempSync(join(tmpdir(), "epic-watch-"));
    const f = join(dir, "k.json");
    await report(f, phase, "--pr", "301", "--note", "n");
    return JSON.parse(readFileSync(f, "utf8")) as Record<string, unknown>;
  }

  test("ready is reported once", async () => {
    const seen: Record<string, Record<string, unknown>> = {};
    const st = await status("ready");
    const first = issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, seen);
    expect(first.map((e) => e.type)).toEqual(["ready"]);
    expect(issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, seen)).toEqual([]);
  });

  test("blocked once per episode", async () => {
    const seen: Record<string, Record<string, unknown>> = {};
    const st = await status("execution");
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

  test("gone only while running and herdr reachable", async () => {
    const st = await status("plan");
    expect(issueEvents("comemory-255", REC, st, {}, {}).map((e) => e.type)).toEqual(["gone"]);
    expect(issueEvents("comemory-255", REC, st, { __error__: "socket" }, {})).toEqual([]);
  });

  test("idle agent with old status is stalled but babysit is patient", async () => {
    const st = { ...(await status("plan")), updated_at: "2026-09-24T08:00:00Z" };
    const types = issueEvents("comemory-255", REC, st, { "comemory-255": "idle" }, {}).map(
      (e) => e.type,
    );
    expect(types).toEqual(["stalled"]);
    const babysitStatus = await status("babysit");
    const freshBabysit = {
      ...st,
      phase: "babysit",
      updated_at: babysitStatus.updated_at,
    };
    expect(issueEvents("comemory-255", REC, freshBabysit, { "comemory-255": "idle" }, {})).toEqual(
      [],
    );
  });
});
