// Real-data tests for the aggregate layer. Builds real git repos with real ledger
// files (so buildState exercises a real git diff_sha), and proves the sidebar
// never parses transcripts (AC-14) via a LiveActivitySource whose tree() throws.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, type DashboardConfig } from "../config.ts";
import { buildState } from "../state.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "../activity/source.ts";
import { buildMultiState, buildSelectedDetail, buildSummaries } from "../aggregate.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-dash-aggregate-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

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
