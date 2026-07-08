// Real-data tests for the multi-project server: real ledger files on disk served
// over HTTP + SSE. The activity source is injected (interface DI; the real source
// is covered in claude-code.test.ts) so this focuses on routing, the multi-project
// payload, project selection, the never-500 contract, and live SSE updates (AC-11).

import { afterAll, afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, type DashboardConfig } from "../config.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "../activity/source.ts";
import { startServer } from "../index.ts";
import type { MultiDashboardState, SessionDetail } from "../aggregate.ts";
import { backfillRepo } from "../activity/backfill.ts";
import type { DiscoveredProject } from "../discovery.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-dash-server-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

const EMPTY: ActivitySummary = { total: 0, running: 0, errored: 0, stale: 0, deepest: 0 };
const inertSource: LiveActivitySource = {
  countSpawned: () => 0,
  tree: () => ({ agents: [] as AgentNode[], summary: { ...EMPTY } }),
  activityFingerprint: () => 0,
};

function writeLedger(name: string, title: string): string {
  const dir = join(base, name, ".claude", "tmp", "plan-ledger");
  mkdirSync(dir, { recursive: true });
  const p = join(dir, "main.json");
  writeFileSync(
    p,
    JSON.stringify({
      version: 1,
      branch: "main",
      base_branch: "main",
      plan_doc: "p.md",
      updated_at: "2026-06-19T00:00:00Z",
      summary: {},
      next: "s1",
      steps: [{ id: "s1", title, check: "true", status: "pending", started_at: null, activity: null, exit_code: null, diff_sha: null, last_run: null, evidence_tail: null }],
    }),
  );
  return p;
}

const cfg = (overrides: Partial<DashboardConfig>): DashboardConfig => ({
  ...DEFAULT_CONFIG,
  roots: [base],
  scanDepth: 2,
  pollMs: 50,
  ...overrides,
});

let server: { port: number; stop: () => void };
afterEach(() => server?.stop());

describe("HTTP routing + payload (AC-11)", () => {
  beforeEach(() => {
    writeLedger("srvA", "ONE");
    server = startServer({ config: cfg({}), source: inertSource });
  });

  test("GET /api/state returns MultiDashboardState with projects + default selection", async () => {
    const res = await fetch(`http://localhost:${server.port}/api/state`);
    expect(res.status).toBe(200);
    const state = (await res.json()) as MultiDashboardState;
    expect(state.projects.length).toBeGreaterThanOrEqual(1);
    expect(state.selected).not.toBeNull();
    expect(state.selected!.plan.ledger?.branch).toBe("main");
    expect(typeof state.serverTime).toBe("string");
  });

  test("?project=<id> selects that project", async () => {
    const all = (await (await fetch(`http://localhost:${server.port}/api/state`)).json()) as MultiDashboardState;
    const id = all.projects[0].id;
    const res = await fetch(`http://localhost:${server.port}/api/state?project=${id}`);
    const state = (await res.json()) as MultiDashboardState;
    expect(state.selected!.id).toBe(id);
  });

  test("GET / serves the SPA", async () => {
    const res = await fetch(`http://localhost:${server.port}/`);
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("text/html");
  });

  test("non-GET → 405, unknown path → 404", async () => {
    expect((await fetch(`http://localhost:${server.port}/api/state`, { method: "POST" })).status).toBe(405);
    expect((await fetch(`http://localhost:${server.port}/nope`)).status).toBe(404);
  });
});

describe("never-500 contract", () => {
  test("empty roots → 200 with empty projects, null selection", async () => {
    server = startServer({ config: cfg({ roots: [] }), source: inertSource });
    const res = await fetch(`http://localhost:${server.port}/api/state`);
    expect(res.status).toBe(200);
    const state = (await res.json()) as MultiDashboardState;
    expect(state.projects).toEqual([]);
    expect(state.selected).toBeNull();
  });
});

// The history route reads the persisted store (not discovery), so we backfill the
// real #117 cc-store fixture into a temp store and ask the live server for it.
describe("GET /api/session (history lane)", () => {
  const FIXTURE_SESSION = "sess-fixture";
  const PROJECTS_ROOT = join(import.meta.dir, "fixtures", "cc-store", "projects");
  const FIX_PROJECT: DiscoveredProject = {
    id: "srvfixproj001",
    root: "/fixture/repo", // slugFor -> -fixture-repo, matching the fixture dir
    branch: "x",
    label: "x",
    ledgerPath: "",
    ledgerMtimeMs: 0,
  };
  const storeDir = mkdtempSync(join(tmpdir(), "toolu-srv-store-"));
  let prevStoreDir: string | undefined;

  beforeEach(() => {
    prevStoreDir = process.env.TOOLU_ACTIVITY_DIR;
    process.env.TOOLU_ACTIVITY_DIR = storeDir;
    backfillRepo(FIX_PROJECT, { projectsRoot: PROJECTS_ROOT });
    server = startServer({ config: cfg({ roots: [] }), source: inertSource });
  });
  afterAll(() => {
    if (prevStoreDir === undefined) delete process.env.TOOLU_ACTIVITY_DIR;
    else process.env.TOOLU_ACTIVITY_DIR = prevStoreDir;
    rmSync(storeDir, { recursive: true, force: true });
  });

  test("returns a real fixture session's tree + summary", async () => {
    const res = await fetch(
      `http://localhost:${server.port}/api/session?project=${FIX_PROJECT.id}&session=${FIXTURE_SESSION}`,
    );
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toContain("application/json");
    const detail = (await res.json()) as SessionDetail;
    expect(detail.tree.length).toBe(3); // 3 top-level agents
    expect(detail.summary.total).toBe(5);
    expect(detail.summary.errored).toBe(1);
  });

  test("missing project or session → 400 JSON", async () => {
    const noSession = await fetch(`http://localhost:${server.port}/api/session?project=${FIX_PROJECT.id}`);
    expect(noSession.status).toBe(400);
    expect((await noSession.json()).error).toBeDefined();
    const noProject = await fetch(`http://localhost:${server.port}/api/session?session=${FIXTURE_SESSION}`);
    expect(noProject.status).toBe(400);
  });

  test("an unknown session yields an empty tree, never a 500", async () => {
    const res = await fetch(
      `http://localhost:${server.port}/api/session?project=${FIX_PROJECT.id}&session=nope`,
    );
    expect(res.status).toBe(200);
    const detail = (await res.json()) as SessionDetail;
    expect(detail.tree).toEqual([]);
    expect(detail.summary.total).toBe(0);
  });

  test("a path-traversal session id → 400 (not 500), reads nothing outside the store", async () => {
    const res = await fetch(
      `http://localhost:${server.port}/api/session?project=${FIX_PROJECT.id}&session=${encodeURIComponent("../escape")}`,
    );
    expect(res.status).toBe(400); // a malformed id is a client error, never a crash
    expect(res.status).not.toBe(500);
    const body = (await res.json()) as Record<string, unknown>;
    // Rejected by the store's validation chokepoint BEFORE any fs read, so the
    // body carries only the rejection — no session payload, no file contents.
    expect(String(body.error)).toContain("unsafe id");
    expect(body).not.toHaveProperty("tree");
    expect(body).not.toHaveProperty("summary");
  });

  test("a path-traversal project id → 400 (not 500)", async () => {
    const res = await fetch(
      `http://localhost:${server.port}/api/session?project=${encodeURIComponent("../..")}&session=${FIXTURE_SESSION}`,
    );
    expect(res.status).toBe(400);
    expect(res.status).not.toBe(500);
    expect((await res.json()).error).toBeDefined();
  });
});

describe("SSE live updates (AC-11/AC-16)", () => {
  test("streams an initial state frame, then a fresh frame after a ledger change", async () => {
    const ledgerPath = writeLedger("srvSSE", "BEFORE");
    server = startServer({ config: cfg({}), source: inertSource });

    const ctrl = new AbortController();
    const res = await fetch(`http://localhost:${server.port}/api/events`, { signal: ctrl.signal });
    const reader = res.body!.getReader();
    const decoder = new TextDecoder();

    const titles: string[] = [];
    const deadline = Date.now() + 4000;
    let buf = "";
    let bumped = false;
    while (Date.now() < deadline && titles.length < 2) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      let i: number;
      while ((i = buf.indexOf("\n\n")) >= 0) {
        const frame = buf.slice(0, i);
        buf = buf.slice(i + 2);
        const dataLine = frame.split("\n").find((l) => l.startsWith("data:"));
        if (!dataLine) continue;
        const state = JSON.parse(dataLine.slice(5).trim()) as MultiDashboardState;
        const t = state.selected?.plan.ledger?.steps[0]?.title;
        if (t) titles.push(t);
        if (!bumped) {
          // after the first real frame, mutate the ledger to trigger a new emit
          bumped = true;
          writeFileSync(ledgerPath, JSON.stringify({ version: 1, branch: "main", base_branch: "main", plan_doc: "p.md", summary: {}, next: "s1", steps: [{ id: "s1", title: "AFTER", check: "true", status: "pending", started_at: null, activity: null, exit_code: null, diff_sha: null, last_run: null, evidence_tail: null }] }));
          const future = new Date(Date.now() + 10_000);
          utimesSync(ledgerPath, future, future);
        }
      }
    }
    ctrl.abort();
    expect(titles[0]).toBe("BEFORE");
    expect(titles).toContain("AFTER");
  });
});

describe("config warning surfacing", () => {
  test("malformed dashboard.json prints a warning to stderr at startup", () => {
    // Regression: loadConfig() computed a warning that no caller read, so a bad
    // config silently reverted roots to [] and the board went empty with no trace.
    // Run in a subprocess so DEFAULT_CONFIG_PATH resolves under our XDG home.
    const xdg = join(base, "xdg-malformed");
    mkdirSync(join(xdg, "toolu"), { recursive: true });
    writeFileSync(join(xdg, "toolu", "dashboard.json"), '{"roots": [,]}');
    const entry = join(import.meta.dir, "..", "index.ts");
    const proc = Bun.spawnSync(
      ["bun", "-e", `import { startServer } from ${JSON.stringify(entry)}; startServer().stop();`],
      { env: { ...process.env, XDG_CONFIG_HOME: xdg } },
    );
    expect(proc.exitCode).toBe(0);
    expect(proc.stderr.toString()).toContain("config json is malformed");
  });
});
