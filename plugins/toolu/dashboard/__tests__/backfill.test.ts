// Real-data tests for the one-shot transcript backfill. Runs against #117's
// captured cc-store fixture (fixtures/cc-store/projects/-fixture-repo/...) by
// pointing the backfill's projectsRoot at it, and persists into a temp store via
// $TOOLU_ACTIVITY_DIR. No mocks. Asserts the persisted store reassembles the same
// tree #117's claude-code.ts builds from the same transcripts (5 agents, nesting,
// statuses), that index.json carries a faithful SessionSummary, and that a 2nd run
// is idempotent (mtime-cached, no duplicate records).

import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { DiscoveredProject } from "../discovery.ts";
import { backfillRepo } from "../activity/backfill.ts";
import { listSessions, projectDir, readSession } from "../activity/store.ts";
import { buildTree } from "../activity/tree-builder.ts";

const PROJECTS_ROOT = join(import.meta.dir, "fixtures", "cc-store", "projects");
const SESSION = "sess-fixture";
const PROJECT_ID = "fixproj00001"; // a #117-shaped 12-char projectId for the store key

// slugFor("/fixture/repo") === "-fixture-repo", matching the fixture dir name.
const PROJECT: DiscoveredProject = {
  id: PROJECT_ID,
  root: "/fixture/repo",
  branch: "x",
  label: "x",
  ledgerPath: "",
  ledgerMtimeMs: 0,
};

// The captured scenario, mirrored from claude-code.test.ts / fixtures/README.md.
const TU_DONE_PARENT = "toolu_01CZjAm4dAWLvaZXKjiFvnCb"; // a4db96, done, 2 children
const TU_RUNNING = "toolu_0172xDeCHo4xPYFm2M8MGb4G"; // a720f8d7, running
const TU_ERROR = "toolu_01QhDS2hKGDLmgBWkztqdz9e"; // a273da, error

const base = mkdtempSync(join(tmpdir(), "toolu-backfill-"));
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

describe("backfillRepo persists the real cc-store fixture", () => {
  test("a fresh backfill writes the session and indexes it", () => {
    const written = backfillRepo(PROJECT, { projectsRoot: PROJECTS_ROOT });
    expect(written).toEqual([SESSION]);

    const sessions = listSessions(PROJECT_ID);
    expect(sessions.length).toBe(1);
    expect(sessions[0]!.sessionId).toBe(SESSION);
    expect(sessions[0]!.agentCount).toBe(5);
    expect(sessions[0]!.errored).toBe(true); // a273da errored
    expect(sessions[0]!.running).toBe(true); // a720f8d7 unresolved
  });

  test("the persisted store rebuilds the same tree #117's buildTree produces", () => {
    backfillRepo(PROJECT, { projectsRoot: PROJECTS_ROOT });
    const { metas, spawns, results } = readSession(PROJECT_ID, SESSION);
    const { agents, summary } = buildTree(metas, spawns, results, 0, 600);

    expect(agents.length).toBe(3);
    const byTool = new Map(agents.map((a) => [a.toolUseId, a]));
    const parent = byTool.get(TU_DONE_PARENT)!;
    expect(parent.status).toBe("done");
    expect(parent.children.length).toBe(2);
    for (const c of parent.children) {
      expect(c.parentToolUseId).toBe(TU_DONE_PARENT);
      expect(c.status).toBe("done");
    }
    expect(byTool.get(TU_RUNNING)!.status).toBe("running");
    expect(byTool.get(TU_ERROR)!.status).toBe("error");

    expect(summary).toEqual({ total: 5, running: 1, errored: 1, stale: 0, deepest: 2 });
  });
});

describe("backfillRepo idempotency", () => {
  test("a 2nd run is a no-op (mtime cache) and produces no duplicate records", () => {
    // Own project id so this is order-independent from the persist tests above.
    const IDEM: DiscoveredProject = { ...PROJECT, id: "idemproj0001" };
    const first = backfillRepo(IDEM, { projectsRoot: PROJECTS_ROOT });
    expect(first).toEqual([SESSION]);
    const logPath = join(projectDir(IDEM.id), `${SESSION}.jsonl`);
    const linesAfterFirst = readFileSync(logPath, "utf8").split("\n").filter((l) => l.trim().length > 0).length;

    const second = backfillRepo(IDEM, { projectsRoot: PROJECTS_ROOT });
    expect(second).toEqual([]); // unchanged mtime → skipped
    const linesAfterSecond = readFileSync(logPath, "utf8").split("\n").filter((l) => l.trim().length > 0).length;
    expect(linesAfterSecond).toBe(linesAfterFirst); // no duplication

    // The reassembled data is unchanged (still 5 metas, 5 spawns, 4 results).
    const data = readSession(IDEM.id, SESSION);
    expect(data.metas.length).toBe(5);
    expect(data.spawns.size).toBe(5);
    expect(data.results.size).toBe(4);
  });

  test("a transcript mtime bump re-backfills the session without dup records", () => {
    const PROJ2: DiscoveredProject = { ...PROJECT, id: "fixproj00002" };
    backfillRepo(PROJ2, { projectsRoot: PROJECTS_ROOT });
    const logPath = join(projectDir(PROJ2.id), `${SESSION}.jsonl`);
    const before = readFileSync(logPath, "utf8");

    // Bump the source transcript's mtime forward so the cache misses.
    const transcript = join(PROJECTS_ROOT, "-fixture-repo", `${SESSION}.jsonl`);
    const orig = statSync(transcript).mtimeMs;
    try {
      const later = new Date(orig + 60_000);
      utimesSync(transcript, later, later);
      const written = backfillRepo(PROJ2, { projectsRoot: PROJECTS_ROOT });
      expect(written).toEqual([SESSION]); // re-backfilled
      // Rewrite (not append): record count identical to the first pass.
      const after = readFileSync(logPath, "utf8");
      const count = (s: string) => s.split("\n").filter((l) => l.trim().length > 0).length;
      expect(count(after)).toBe(count(before));
    } finally {
      const restore = new Date(orig);
      utimesSync(transcript, restore, restore); // leave the shared fixture pristine
    }
  });
});

describe("backfillRepo graceful degradation", () => {
  test("a project with no transcripts under the root returns [] and writes nothing", () => {
    const NONE: DiscoveredProject = { ...PROJECT, id: "noproj000001", root: "/no/such/repo" };
    expect(backfillRepo(NONE, { projectsRoot: PROJECTS_ROOT })).toEqual([]);
    expect(listSessions("noproj000001")).toEqual([]);
  });
});

describe("backfillRepo enforces promptPreviewChars", () => {
  // Self-contained temp transcript root (the shared cc-store fixture is left
  // pristine): one main transcript spawning one subagent whose meta sidecar
  // carries a description far longer than the cap.
  const CAP = 120; // default promptPreviewChars (no dashboard.json in the test env)
  const LONG = "Z".repeat(CAP + 200);
  const TOOL_ID = "toolu_backfill_truncate_1";
  const AGENT_ID = "a1b2c3d4e5f60718";
  const ROOT = mkdtempSync(join(tmpdir(), "toolu-backfill-trunc-"));
  // slugFor("/trunc/repo") === "-trunc-repo"
  const TRUNC_PROJECT: DiscoveredProject = { ...PROJECT, id: "truncproj001", root: "/trunc/repo" };

  beforeEach(() => {
    const slugDir = join(ROOT, "-trunc-repo");
    const subDir = join(slugDir, SESSION, "subagents");
    mkdirSync(subDir, { recursive: true });
    writeFileSync(
      join(slugDir, `${SESSION}.jsonl`),
      JSON.stringify({
        type: "assistant",
        timestamp: "2026-06-20T10:00:00.000Z",
        message: { content: [{ type: "tool_use", id: TOOL_ID, name: "Agent", input: { description: LONG, subagent_type: "general-purpose" } }] },
      }) + "\n",
    );
    writeFileSync(
      join(subDir, `agent-${AGENT_ID}.meta.json`),
      JSON.stringify({ agentType: "general-purpose", description: LONG, toolUseId: TOOL_ID }),
    );
  });
  afterAll(() => rmSync(ROOT, { recursive: true, force: true }));

  test("a meta description over the cap is persisted truncated; full text absent from the .jsonl", () => {
    const written = backfillRepo(TRUNC_PROJECT, { projectsRoot: ROOT });
    expect(written).toEqual([SESSION]);

    const data = readSession(TRUNC_PROJECT.id, SESSION);
    const meta = data.metas.find((m) => m.toolUseId === TOOL_ID)!;
    expect(meta.description.length).toBe(CAP);
    expect(meta.description).toBe("Z".repeat(CAP));

    const raw = readFileSync(join(projectDir(TRUNC_PROJECT.id), `${SESSION}.jsonl`), "utf8");
    expect(raw.includes(LONG)).toBe(false); // the full original text was never persisted
  });
});
