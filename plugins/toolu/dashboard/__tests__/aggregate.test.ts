// Real-data tests for the aggregate layer. Builds real git repos with real ledger
// files (so buildState exercises a real git diff_sha), and proves the sidebar
// never parses transcripts (AC-14) via a LiveActivitySource whose tree() throws.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";

import { DEFAULT_CONFIG, type DashboardConfig } from "../config.ts";
import { buildState } from "../state.ts";
import { slugFor } from "../activity/claude-code.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "../activity/source.ts";
import { buildMultiState, buildSelectedDetail, buildSessionDetail, buildSummaries } from "../aggregate.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-dash-aggregate-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

/** Newest event timestamp (ms) across every .jsonl under a cc-store fixture dir
 *  (session log + its subagents), derived at runtime so a test clock anchored to
 *  it tracks the fixture instead of a hand-copied literal that could silently
 *  drift. Throws if the fixture yields no parseable timestamp — a broken fixture
 *  must fail loudly, not hand back a bogus anchor. */
function latestFixtureEventMs(fixtureDir: string): number {
  let max = Number.NEGATIVE_INFINITY;
  for (const entry of readdirSync(fixtureDir, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile() || !entry.name.endsWith(".jsonl")) continue;
    const body = readFileSync(join(entry.parentPath, entry.name), "utf8");
    for (const m of body.matchAll(/"timestamp":"([^"]+)"/g)) {
      const ms = Date.parse(m[1]);
      if (!Number.isNaN(ms) && ms > max) max = ms;
    }
  }
  if (max === Number.NEGATIVE_INFINITY) {
    throw new Error(`no parseable "timestamp" in any .jsonl under ${fixtureDir}`);
  }
  return max;
}

/** Create a real git repo (branch `main`, one commit) with a real ledger. */
function makeRepo(name: string, steps: unknown[]): string {
  const root = join(base, name);
  mkdirSync(root, { recursive: true });
  const git = (...args: string[]) => execFileSync("git", ["-C", root, ...args], { stdio: "ignore" });
  git("init", "-b", "main");
  git("config", "user.email", "t@t.io");
  git("config", "user.name", "t");
  git("commit", "--allow-empty", "-m", "init");
  const dir = join(root, ".claude", "tmp", "plan-ledger");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "main.json"),
    JSON.stringify({
      version: 1,
      branch: "main",
      base_branch: "main",
      plan_doc: "p.md",
      updated_at: "2026-06-19T00:00:00Z",
      summary: { stale: 2 },
      next: null,
      steps,
    }),
  );
  return root;
}

const EMPTY: ActivitySummary = { total: 0, running: 0, errored: 0, stale: 0, deepest: 0 };
/** A trivial real-shaped source (DI at the interface boundary; real ledgers/git still flow through). */
const inertSource: LiveActivitySource = {
  countSpawned: () => 3,
  tree: () => ({ agents: [] as AgentNode[], summary: { ...EMPTY } }),
};

let cfg: DashboardConfig;
let rootA: string;
beforeAll(() => {
  rootA = makeRepo("repoA", [
    { id: "s1", title: "t", check: "true", status: "green", started_at: null, activity: null, exit_code: 0, diff_sha: "x", last_run: null, evidence_tail: null },
    { id: "s2", title: "t", check: "true", status: "running", started_at: "2026-06-19T00:00:00Z", activity: "go", exit_code: null, diff_sha: null, last_run: null, evidence_tail: null },
  ]);
  makeRepo("repoB", []);
  cfg = { ...DEFAULT_CONFIG, roots: [base], scanDepth: 2 };
});

describe("buildSummaries", () => {
  test("returns a summary per active project with plan counts, dot, agentsSpawned", () => {
    const summaries = buildSummaries(cfg, inertSource);
    const a = summaries.find((s) => s.root === rootA)!;
    expect(a.planCounts).toEqual({ pending: 0, running: 1, red: 0, green: 1 });
    expect(a.dot).toBe("running");
    expect(a.planStale).toBe(2); // read cheaply from ledger.summary, not computed
    expect(a.agentsSpawned).toBe(3);
  });

  test("AC-14: never parses transcripts — tree() is not called", () => {
    let treeCalls = 0;
    const spy: LiveActivitySource = {
      countSpawned: () => {
        return 1;
      },
      tree: () => {
        treeCalls++;
        throw new Error("buildSummaries must not call tree()");
      },
    };
    expect(() => buildSummaries(cfg, spy)).not.toThrow();
    expect(treeCalls).toBe(0);
  });
});

describe("buildSelectedDetail (AC-4, AC-11)", () => {
  test("plan equals buildState for the same ledger+root (regression)", () => {
    const summaries = buildSummaries(cfg, inertSource);
    const idA = summaries.find((s) => s.root === rootA)!.id;
    const detail = buildSelectedDetail(cfg, inertSource, idA, Date.now())!;
    const direct = buildState({ ledgerPath: join(rootA, ".claude", "tmp", "plan-ledger", "main.json"), repoRoot: rootA, stuckThresholdSeconds: cfg.stuckThresholdSeconds });
    // serverTime is a per-call timestamp; compare the load-bearing fields
    expect(detail.plan.ledger).toEqual(direct.ledger);
    expect(detail.plan.currentDiffSha).toEqual(direct.currentDiffSha);
    expect(detail.plan.steps).toEqual(direct.steps);
  });

  test("AC-11: unknown id → null", () => {
    expect(buildSelectedDetail(cfg, inertSource, "deadbeef", Date.now())).toBeNull();
  });

  test("the selected lane DOES use the activity source (tree throws ⟹ surfaced)", () => {
    const summaries = buildSummaries(cfg, inertSource);
    const idA = summaries.find((s) => s.root === rootA)!.id;
    const throwing: LiveActivitySource = {
      countSpawned: () => 0,
      tree: () => {
        throw new Error("tree called");
      },
    };
    expect(() => buildSelectedDetail(cfg, throwing, idA, Date.now())).toThrow("tree called");
  });
});

describe("buildMultiState", () => {
  test("defaults selection to the most-recently-active project", () => {
    const state = buildMultiState(cfg, inertSource, null, Date.now());
    expect(state.projects.length).toBeGreaterThanOrEqual(1);
    expect(state.selected).not.toBeNull();
    expect(state.selected!.id).toBe(state.projects[0].id);
    expect(typeof state.serverTime).toBe("string");
  });
});

// Real-data history wiring: a real discovered git repo whose Claude Code slug
// dir is the #117 cc-store fixture session (copied so slugFor(root) matches),
// proving buildSelectedDetail backfills + attaches sessions, and that
// buildSessionDetail reassembles that exact past run's tree.
describe("buildSelectedDetail attaches persisted sessions (history lane)", () => {
  const FIXTURE_SESSION = "sess-fixture";
  const FIXTURE_CC = join(import.meta.dir, "fixtures", "cc-store", "projects", "-fixture-repo");
  const histBase = mkdtempSync(join(tmpdir(), "toolu-dash-history-"));
  const cfgDir = join(histBase, "cc"); // serves as $CLAUDE_CONFIG_DIR
  const storeDir = join(histBase, "store"); // serves as $TOOLU_ACTIVITY_DIR
  let histRoot: string;
  let histCfg: DashboardConfig;
  let prevCfgDir: string | undefined;
  let prevStoreDir: string | undefined;

  beforeAll(() => {
    histRoot = makeRepo("histRepo", [
      { id: "s1", title: "t", check: "true", status: "green", started_at: null, activity: null, exit_code: 0, diff_sha: "x", last_run: null, evidence_tail: null },
    ]);
    // Drop the real fixture transcript + subagents into the slug dir for this repo.
    const slugDir = join(cfgDir, "projects", slugFor(histRoot));
    mkdirSync(slugDir, { recursive: true });
    cpSync(join(FIXTURE_CC, `${FIXTURE_SESSION}.jsonl`), join(slugDir, `${FIXTURE_SESSION}.jsonl`));
    cpSync(join(FIXTURE_CC, FIXTURE_SESSION), join(slugDir, FIXTURE_SESSION), { recursive: true });

    prevCfgDir = process.env.CLAUDE_CONFIG_DIR;
    prevStoreDir = process.env.TOOLU_ACTIVITY_DIR;
    process.env.CLAUDE_CONFIG_DIR = cfgDir;
    process.env.TOOLU_ACTIVITY_DIR = storeDir;
    // makeRepo creates the repo under the module-level `base`, so scan that.
    histCfg = { ...DEFAULT_CONFIG, roots: [base], scanDepth: 3 };
  });

  afterAll(() => {
    if (prevCfgDir === undefined) delete process.env.CLAUDE_CONFIG_DIR;
    else process.env.CLAUDE_CONFIG_DIR = prevCfgDir;
    if (prevStoreDir === undefined) delete process.env.TOOLU_ACTIVITY_DIR;
    else process.env.TOOLU_ACTIVITY_DIR = prevStoreDir;
    rmSync(histBase, { recursive: true, force: true });
  });

  // syncSessions prunes by DEFAULT_CONFIG.retentionDays (30) measured from the
  // clock passed in. Passing the real Date.now() made these tests a time bomb:
  // they went red once the fixture's events aged past the window and
  // applyRetention deleted the session mid-test. Anchor the clock to the fixture
  // itself — the lane under test is retention-independent, so pinning it is the
  // point. Derive the anchor from the fixture's own newest event at runtime
  // (not a hardcoded literal) so it stays correct if the fixture is ever
  // re-dated, and never drifts with the wall clock.
  const FIXTURE_NOW = latestFixtureEventMs(FIXTURE_CC) + 1000;

  test("sessions are populated after backfilling the fixture project", () => {
    const id = buildSummaries(histCfg, inertSource).find((s) => basename(s.root) === "histRepo")!.id;
    const detail = buildSelectedDetail(histCfg, inertSource, id, FIXTURE_NOW)!;
    expect(detail.sessions.length).toBe(1);
    expect(detail.sessions[0].sessionId).toBe(FIXTURE_SESSION);
    expect(detail.sessions[0].agentCount).toBe(5);
    expect(detail.sessions[0].errored).toBe(true);
    // live lane is untouched (the inert source yields an empty live tree)
    expect(detail.agents).toEqual([]);
  });

  test("buildSessionDetail reassembles that past run's tree", () => {
    const id = buildSummaries(histCfg, inertSource).find((s) => basename(s.root) === "histRepo")!.id;
    buildSelectedDetail(histCfg, inertSource, id, FIXTURE_NOW); // ensure persisted
    const { tree, summary } = buildSessionDetail(id, FIXTURE_SESSION, 0, 600);
    expect(tree.length).toBe(3); // 3 top-level agents in the fixture
    expect(summary.total).toBe(5);
    expect(summary.errored).toBe(1);
  });
});
