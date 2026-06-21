// Real-data tests for the persistent activity store: seeds a temp store by
// pointing $TOOLU_ACTIVITY_DIR at a temp dir and appending real records, then
// asserts list/read/retention/torn-line tolerance. No mocks. The decisive test
// feeds readSession straight into #117's buildTree and asserts the assembled tree
// matches the captured cc-store fixture scenario (5 agents, nesting, statuses).

import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildTree } from "../activity/tree-builder.ts";
import {
  type ActivityRecord,
  appendRecord,
  applyRetention,
  assertSafeId,
  activityBaseDir,
  listSessions,
  projectDir,
  readSession,
  storeHasProject,
  summarizeSession,
  writeIndex,
} from "../activity/store.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-store-"));
const PROJ = "abc123def456"; // a #117-shaped 12-char projectId

let prevDir: string | undefined;
beforeEach(() => {
  prevDir = process.env.TOOLU_ACTIVITY_DIR;
  process.env.TOOLU_ACTIVITY_DIR = base;
});
afterAll(() => {
  if (prevDir === undefined) delete process.env.TOOLU_ACTIVITY_DIR;
  else process.env.TOOLU_ACTIVITY_DIR = prevDir;
  rmSync(base, { recursive: true, force: true });
});

/** The captured cc-store scenario as store records (verbatim ids/timestamps). */
const FIXTURE_RECORDS: ActivityRecord[] = [
  // top-level a4db96 (done) → children ab647f22, a8eb8e23 (both done)
  { kind: "meta", agentId: "a4db96c6ed495e432", agentType: "general-purpose", description: "Map dashboard backend architecture", toolUseId: "toolu_01CZjAm4dAWLvaZXKjiFvnCb" },
  { kind: "meta", agentId: "ab647f22b4cc8d7b4", agentType: "general-purpose", description: "Extract detect.sh functions", toolUseId: "toolu_01MqCvpJ3aE2Sg74sfrbk5vo" },
  { kind: "meta", agentId: "a8eb8e2388dbabd77", agentType: "general-purpose", description: "Map frontend assets", toolUseId: "toolu_01TgXWnested2" },
  // top-level a720f8d7 (running — no result) and a273da (error)
  { kind: "meta", agentId: "a720f8d7e5ef4e047", agentType: "general-purpose", description: "Find the dashboard", toolUseId: "toolu_0172xDeCHo4xPYFm2M8MGb4G" },
  { kind: "meta", agentId: "a273da87215e87146", agentType: "general-purpose", description: "Map dashboard frontend UX", toolUseId: "toolu_01QhDS2hKGDLmgBWkztqdz9e" },
  { kind: "spawn", toolUseId: "toolu_01CZjAm4dAWLvaZXKjiFvnCb", startedAt: "2026-06-20T10:00:00.000Z", ownerAgentId: null },
  { kind: "spawn", toolUseId: "toolu_0172xDeCHo4xPYFm2M8MGb4G", startedAt: "2026-06-20T10:00:05.000Z", ownerAgentId: null },
  { kind: "spawn", toolUseId: "toolu_01QhDS2hKGDLmgBWkztqdz9e", startedAt: "2026-06-20T10:00:10.000Z", ownerAgentId: null },
  { kind: "spawn", toolUseId: "toolu_01MqCvpJ3aE2Sg74sfrbk5vo", startedAt: "2026-06-20T10:00:01.000Z", ownerAgentId: "a4db96c6ed495e432" },
  { kind: "spawn", toolUseId: "toolu_01TgXWnested2", startedAt: "2026-06-20T10:00:02.000Z", ownerAgentId: "a4db96c6ed495e432" },
  { kind: "result", toolUseId: "toolu_01CZjAm4dAWLvaZXKjiFvnCb", endedAt: "2026-06-20T10:01:00.000Z", isError: false },
  { kind: "result", toolUseId: "toolu_01QhDS2hKGDLmgBWkztqdz9e", endedAt: "2026-06-20T10:01:10.000Z", isError: true },
  { kind: "result", toolUseId: "toolu_01MqCvpJ3aE2Sg74sfrbk5vo", endedAt: "2026-06-20T10:00:30.000Z", isError: false },
  { kind: "result", toolUseId: "toolu_01TgXWnested2", endedAt: "2026-06-20T10:00:40.000Z", isError: false },
  // a720f8d7 has no result → running
];

function seedSession(projectId: string, sessionId: string, recs: ActivityRecord[]): void {
  for (const r of recs) appendRecord(projectId, sessionId, r);
}

describe("activityBaseDir / projectDir", () => {
  test("honours $TOOLU_ACTIVITY_DIR and nests by projectId", () => {
    expect(activityBaseDir()).toBe(base);
    expect(projectDir(PROJ)).toBe(join(base, PROJ));
  });

  test("falls back to ${CLAUDE_CONFIG_DIR}/toolu/activity when env unset", () => {
    delete process.env.TOOLU_ACTIVITY_DIR;
    const prevCfg = process.env.CLAUDE_CONFIG_DIR;
    process.env.CLAUDE_CONFIG_DIR = "/tmp/cfg";
    expect(activityBaseDir()).toBe(join("/tmp/cfg", "toolu", "activity"));
    if (prevCfg === undefined) delete process.env.CLAUDE_CONFIG_DIR;
    else process.env.CLAUDE_CONFIG_DIR = prevCfg;
  });
});

describe("appendRecord + readSession", () => {
  test("a fresh project has no sessions and is absent", () => {
    expect(listSessions("nope")).toEqual([]);
    expect(storeHasProject("nope")).toBe(false);
    expect(readSession("nope", "none")).toEqual({ metas: [], spawns: new Map(), results: new Map() });
  });

  test("appended records reassemble into {metas, spawns, results}", () => {
    seedSession(PROJ, "sess-1", FIXTURE_RECORDS);
    expect(storeHasProject(PROJ)).toBe(true);
    const data = readSession(PROJ, "sess-1");
    expect(data.metas.length).toBe(5);
    expect(data.spawns.size).toBe(5);
    expect(data.results.size).toBe(4); // a720f8d7 has no result
    expect(data.spawns.get("toolu_01MqCvpJ3aE2Sg74sfrbk5vo")?.ownerAgentId).toBe("a4db96c6ed495e432");
    expect(data.results.get("toolu_01QhDS2hKGDLmgBWkztqdz9e")?.isError).toBe(true);
  });
});

describe("readSession feeds #117's buildTree", () => {
  test("the assembled tree matches the captured scenario", () => {
    seedSession(PROJ, "sess-tree", FIXTURE_RECORDS);
    const { metas, spawns, results } = readSession(PROJ, "sess-tree");
    const { agents, summary } = buildTree(metas, spawns, results, 0, 600);

    expect(agents.length).toBe(3); // a4db96 (done), a720f8d7 (running), a273da (error)
    const byTool = new Map(agents.map((a) => [a.toolUseId, a]));
    const parent = byTool.get("toolu_01CZjAm4dAWLvaZXKjiFvnCb")!;
    expect(parent.status).toBe("done");
    expect(parent.children.length).toBe(2);
    for (const c of parent.children) expect(c.parentToolUseId).toBe("toolu_01CZjAm4dAWLvaZXKjiFvnCb");
    expect(byTool.get("toolu_0172xDeCHo4xPYFm2M8MGb4G")!.status).toBe("running");
    expect(byTool.get("toolu_01QhDS2hKGDLmgBWkztqdz9e")!.status).toBe("error");

    expect(summary.total).toBe(5);
    expect(summary.running).toBe(1);
    expect(summary.errored).toBe(1);
    expect(summary.deepest).toBe(2);
  });
});

describe("readSession torn-line tolerance", () => {
  test("a truncated trailing line is skipped, not thrown", () => {
    seedSession(PROJ, "sess-torn", FIXTURE_RECORDS);
    // Append a deliberately torn (partial JSON) trailing line, exactly as an
    // interrupted append would leave the file.
    appendFileSync(join(projectDir(PROJ), "sess-torn.jsonl"), '{"kind":"meta","agentId":"trunc');
    const data = readSession(PROJ, "sess-torn");
    expect(data.metas.length).toBe(5); // the torn meta is dropped, the good five remain
    const { summary } = buildTree(data.metas, data.spawns, data.results, 0, 600);
    expect(summary.total).toBe(5);
  });

  test("a blank/garbage interior line is skipped too", () => {
    const dir = projectDir(PROJ);
    mkdirSync(dir, { recursive: true });
    writeFileSync(
      join(dir, "sess-garbage.jsonl"),
      [
        JSON.stringify({ kind: "meta", agentId: "x", agentType: "t", description: "d", toolUseId: "toolu_x" }),
        "",
        "not json at all",
        JSON.stringify({ kind: "spawn", toolUseId: "toolu_x", startedAt: "2026-06-20T10:00:00.000Z", ownerAgentId: null }),
      ].join("\n") + "\n",
    );
    const data = readSession(PROJ, "sess-garbage");
    expect(data.metas.length).toBe(1);
    expect(data.spawns.size).toBe(1);
  });
});

describe("listSessions + index.json", () => {
  test("listSessions reads index.json (no fs scan); summarizeSession is faithful", () => {
    seedSession(PROJ, "sess-idx", FIXTURE_RECORDS);
    const summary = summarizeSession("sess-idx", readSession(PROJ, "sess-idx"));
    expect(summary.agentCount).toBe(5);
    expect(summary.errored).toBe(true);
    expect(summary.running).toBe(true); // a720f8d7 unresolved
    expect(summary.startedAt).toBe("2026-06-20T10:00:00.000Z");
    writeIndex(PROJ, { sessions: [summary] });
    const listed = listSessions(PROJ);
    expect(listed.length).toBe(1);
    expect(listed[0]).toEqual(summary);
  });
});

describe("applyRetention", () => {
  const RPROJ = "ret000111222";
  function summary(sessionId: string, endedAt: string): ReturnType<typeof summarizeSession> {
    return { sessionId, startedAt: endedAt, endedAt, agentCount: 1, errored: false, running: false };
  }

  test("drops aged sessions and deletes their logs", () => {
    const old = "2000-01-01T00:00:00.000Z";
    const fresh = new Date().toISOString();
    appendRecord(RPROJ, "old-sess", { kind: "meta", agentId: "o", agentType: "t", description: "d", toolUseId: "toolu_o" });
    appendRecord(RPROJ, "fresh-sess", { kind: "meta", agentId: "f", agentType: "t", description: "d", toolUseId: "toolu_f" });
    writeIndex(RPROJ, { sessions: [summary("old-sess", old), summary("fresh-sess", fresh)] });

    const removed = applyRetention(RPROJ, { retentionDays: 30, maxSessionsPerProject: 200 });
    expect(removed).toEqual(["old-sess"]);
    expect(listSessions(RPROJ).map((s) => s.sessionId)).toEqual(["fresh-sess"]);
    expect(readSession(RPROJ, "old-sess").metas.length).toBe(0); // log deleted
    expect(readSession(RPROJ, "fresh-sess").metas.length).toBe(1); // kept
  });

  test("caps at maxSessionsPerProject, keeping the newest", () => {
    const CAP = "cap000111222";
    const now = Date.now();
    const sessions = [];
    for (let i = 0; i < 5; i++) {
      const id = `s${i}`;
      const ts = new Date(now - i * 1000).toISOString(); // s0 newest, s4 oldest
      appendRecord(CAP, id, { kind: "meta", agentId: id, agentType: "t", description: "d", toolUseId: `toolu_${id}` });
      sessions.push(summary(id, ts));
    }
    writeIndex(CAP, { sessions });
    const removed = applyRetention(CAP, { retentionDays: 365_000, maxSessionsPerProject: 2 });
    expect(removed.sort()).toEqual(["s2", "s3", "s4"]);
    expect(listSessions(CAP).map((s) => s.sessionId).sort()).toEqual(["s0", "s1"]);
  });

  test("no-op when index absent", () => {
    expect(applyRetention("absent999", { retentionDays: 1, maxSessionsPerProject: 1 })).toEqual([]);
  });
});

describe("assertSafeId (path-traversal chokepoint)", () => {
  test("rejects empty, '.', '..', '..'-bearing, separators, and NUL", () => {
    for (const bad of ["", ".", "..", "../escape", "..\\escape", "a/b", "a\\b", "a..b", "foo/../bar", "x\0y", ".hidden"]) {
      expect(() => assertSafeId(bad)).toThrow();
    }
  });

  test("accepts a real 12-hex projectId and a real uuid sessionId", () => {
    const projectId = "abc123def456"; // a #117-shaped 12-hex projectId
    const sessionId = "0172xDeC-Ho4x-PYFm-2M8M-Gb4G5e1f9a2b"; // a uuid/hex-dash sessionId
    expect(assertSafeId(projectId)).toBe(projectId);
    expect(assertSafeId(sessionId)).toBe(sessionId);
    expect(assertSafeId("sess-fixture")).toBe("sess-fixture");
  });

  test("a malformed id makes the path-deriving store fns throw, not escape", () => {
    expect(() => projectDir("../other")).toThrow();
    // readSession validates before the fs read, so traversal is a hard error
    // (not a silently-empty read like a genuinely missing file would be).
    expect(() => readSession(PROJ, "../../etc/passwd")).toThrow();
  });
});

describe("promptPreviewChars truncation at the write chokepoint", () => {
  test("appendRecord stores a description truncated to the cap; full text absent on disk", () => {
    const sessionId = "sess-truncate";
    // Default promptPreviewChars is 120 (no dashboard.json in the test env).
    const CAP = 120;
    const longDescription = "P".repeat(CAP + 80); // well over the cap
    appendRecord(PROJ, sessionId, {
      kind: "meta",
      agentId: "trunc-agent",
      agentType: "general-purpose",
      description: longDescription,
      toolUseId: "toolu_truncate_1",
    });

    const data = readSession(PROJ, sessionId);
    const meta = data.metas.find((m) => m.agentId === "trunc-agent")!;
    expect(meta.description.length).toBe(CAP);
    expect(meta.description).toBe("P".repeat(CAP));

    // The full original text must NOT be present anywhere in the on-disk .jsonl.
    const raw = readFileSync(join(projectDir(PROJ), `${sessionId}.jsonl`), "utf8");
    expect(raw.includes(longDescription)).toBe(false);
  });
});
