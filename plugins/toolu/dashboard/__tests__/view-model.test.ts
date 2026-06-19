// Tests the pure view-model against real data: a real ledger (via buildState) for
// the plan helpers, and the real captured cc-store agent tree (via the real
// activity adapter) for tree flattening. No mocks, no hand-built shapes.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { DiscoveredProject } from "../discovery.ts";
import { buildState, type DerivedStep } from "../state.ts";
import { claudeCodeSource } from "../activity/claude-code.ts";
import { columnize, formatDuration, planTotals, treeRows } from "../public/view-model.js";

const STORE = join(import.meta.dir, "fixtures", "cc-store");
const base = mkdtempSync(join(tmpdir(), "toolu-dash-vm-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

function project(root: string): DiscoveredProject {
  return { id: "t", root, branch: "x", label: "x", ledgerPath: "", ledgerMtimeMs: 0 };
}

let steps: DerivedStep[];
let prevCfg: string | undefined;
beforeAll(() => {
  prevCfg = process.env.CLAUDE_CONFIG_DIR;
  process.env.CLAUDE_CONFIG_DIR = STORE;
  const dir = join(base, "repo", ".claude", "tmp", "plan-ledger");
  mkdirSync(dir, { recursive: true });
  const ledgerPath = join(dir, "main.json");
  writeFileSync(
    ledgerPath,
    JSON.stringify({ version: 1, branch: "main", base_branch: "main", plan_doc: "p.md", summary: {}, next: null, steps: [
      { id: "s1", title: "a", check: "true", status: "green", started_at: null, activity: null, exit_code: 0, diff_sha: null, last_run: null, evidence_tail: null },
      { id: "s2", title: "b", check: "true", status: "running", started_at: "2026-06-19T00:00:00Z", activity: "x", exit_code: null, diff_sha: null, last_run: null, evidence_tail: null },
    ] }),
  );
  steps = buildState({ ledgerPath, repoRoot: join(base, "repo") }).steps;
});
afterAll(() => {
  if (prevCfg === undefined) delete process.env.CLAUDE_CONFIG_DIR;
  else process.env.CLAUDE_CONFIG_DIR = prevCfg;
});

describe("columnize + planTotals on real derived steps", () => {
  test("groups derived steps into the four columns", () => {
    const cols = columnize(steps);
    expect(cols.green.length).toBe(1);
    expect(cols.running.length).toBe(1);
    expect(cols.pending.length).toBe(0);
  });

  test("plan totals reflect counts", () => {
    expect(planTotals({ pending: 0, running: 1, red: 0, green: 1 })).toEqual({ total: 2, donePct: 50 });
    expect(planTotals({ pending: 0, running: 0, red: 0, green: 0 })).toEqual({ total: 0, donePct: 0 });
  });
});

describe("treeRows on the real captured agent tree", () => {
  test("flattens preorder with depth; a child follows its parent", () => {
    const { agents } = claudeCodeSource.tree(project("/fixture/repo"), 0, 600);
    const rows = treeRows(agents);
    expect(rows.length).toBe(5); // 3 roots + 2 nested children
    expect(rows.some((r) => r.depth >= 1)).toBe(true);
    const parentIdx = rows.findIndex((r) => r.node.children.length > 0);
    expect(rows[parentIdx + 1].depth).toBe(rows[parentIdx].depth + 1);
  });
});

describe("formatDuration", () => {
  test("formats sub-minute, minutes, hours, and null", () => {
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(1400)).toBe("1.4s");
    expect(formatDuration(125_000)).toBe("2m 05s");
    expect(formatDuration(3_780_000)).toBe("1h 03m");
  });
});
