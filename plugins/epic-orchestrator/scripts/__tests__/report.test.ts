/** The real report.ts writing the status files the watcher reads. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { issueEvents } from "../epic-watch.ts";

const REPORT = join(import.meta.dir, "..", "report.ts");

type Status = {
  phase: string;
  pr: number | null;
  note: string | null;
  updated_at: string;
  history: { phase: string; at: string; note: string | null }[];
};

const report = (statusFile: string, ...args: string[]) => run(["bun", REPORT, statusFile, ...args]);

const readStatus = (file: string) => JSON.parse(readFileSync(file, "utf8")) as Status;

test.concurrent("history and pr persist across phases", async () => {
  using sb = createSandbox();
  const f = join(sb.root, "status", "k.json");
  expect((await report(f, "brainstorm")).exitCode).toBe(0);
  expect((await report(f, "spec")).exitCode).toBe(0);
  const opened = await report(f, "pr-open", "--pr", "301");
  expect(opened.exitCode).toBe(0);
  expect(opened.stdout).toBe("reported pr-open (PR #301)\n");
  expect((await report(f, "babysit")).exitCode).toBe(0);
  const data = readStatus(f);
  expect(data.phase).toBe("babysit");
  expect(data.pr).toBe(301);
  expect(data.note).toBeNull();
  expect(data.updated_at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
  expect(data.history.map((h) => h.phase)).toEqual(["brainstorm", "spec", "pr-open", "babysit"]);
  expect(Object.keys(data)).toEqual(["phase", "pr", "note", "updated_at", "history"]);
});

test.concurrent("a note lands on the status and its history entry, then clears", async () => {
  using sb = createSandbox();
  const f = join(sb.root, "k.json");
  const res = await report(f, "needs-human", "--note", "which base branch?");
  expect(res.stdout).toBe("reported needs-human\n");
  expect(readStatus(f).note).toBe("which base branch?");
  expect(readStatus(f).history[0]?.note).toBe("which base branch?");
  await report(f, "spec");
  expect(readStatus(f).note).toBeNull();
  expect(readStatus(f).history[1]?.note).toBeNull();
});

test.concurrent("the watcher reads what report.ts writes", async () => {
  using sb = createSandbox();
  const f = join(sb.root, "status", "comemory-255.json");
  await report(f, "ready", "--pr", "301");
  const rec = { ref: "Falconiere/comemory#255", agent: "comemory-255", stage: "running" };
  const events = issueEvents("comemory-255", rec, readStatus(f), { "comemory-255": "idle" }, {});
  expect(events.map((e) => e.type)).toEqual(["ready"]);
  expect(events[0]?.pr).toBe(301);
});

test.concurrent("keys a previous status carried keep their place", async () => {
  using sb = createSandbox();
  const f = sb.write("k.json", { extra: "kept", phase: "spec", pr: 7, history: [] });
  await report(f, "plan");
  const data = readStatus(f) as Status & { extra: string };
  expect(Object.keys(data)).toEqual(["extra", "phase", "pr", "history", "note", "updated_at"]);
  expect(data.extra).toBe("kept");
  expect(data.pr).toBe(7);
});

test.concurrent("a corrupt or non-object previous status starts over", async () => {
  using sb = createSandbox();
  for (const body of ["{not json", "[1,2]", ""]) {
    const f = join(sb.root, "k.json");
    writeFileSync(f, body);
    expect((await report(f, "spec")).exitCode).toBe(0);
    expect(readStatus(f).history.map((h) => h.phase)).toEqual(["spec"]);
  }
});

test.concurrent("usage errors exit 2 and write nothing", async () => {
  using sb = createSandbox();
  const f = join(sb.root, "k.json");
  const cases: [string[], string][] = [
    [["merged"], "unknown phase: merged"],
    [["ready", "--pr", "abc"], "--pr must be a number"],
    [["ready", "--pr"], "--pr needs a number"],
    [["ready", "--note"], "--note needs text"],
    [["ready", "--bogus"], "usage: report.ts"],
  ];
  for (const [args, message] of cases) {
    const res = await report(f, ...args);
    expect(res.exitCode).toBe(2);
    expect(res.stderr).toContain(message);
  }
  const tooFew = await run(["bun", REPORT, f]);
  expect(tooFew.exitCode).toBe(2);
  expect(tooFew.stderr).toContain("usage: report.ts");
  expect(existsSync(f)).toBe(false);
});
