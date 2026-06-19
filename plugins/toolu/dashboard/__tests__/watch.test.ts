// Real-data tests for the watch driver. Uses real on-disk ledger files and a
// counting activity source to prove: a ledger mutation yields a fresh emit, and
// an unchanged tick re-parses nothing (AC-16). tick() is driven directly so the
// test is deterministic — no reliance on wall-clock timers.

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, type DashboardConfig } from "../config.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "../activity/source.ts";
import { createWatcher } from "../watch.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-dash-watch-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

function ledger(name: string): string {
  const dir = join(base, name, ".claude", "tmp", "plan-ledger");
  mkdirSync(dir, { recursive: true });
  const p = join(dir, "main.json");
  writeFileSync(p, JSON.stringify({ version: 1, branch: "main", base_branch: "main", plan_doc: "p.md", summary: {}, next: null, steps: [] }));
  return p;
}

const cfg: DashboardConfig = { ...DEFAULT_CONFIG, roots: [base], scanDepth: 2 };

const EMPTY: ActivitySummary = { total: 0, running: 0, errored: 0, stale: 0, deepest: 0 };
/** Counting source: records how many times the deep tree() build runs. */
function countingSource(): { src: LiveActivitySource; treeCalls: () => number } {
  let calls = 0;
  return {
    treeCalls: () => calls,
    src: {
      countSpawned: () => 0,
      tree: () => {
        calls++;
        return { agents: [] as AgentNode[], summary: { ...EMPTY } };
      },
      activityFingerprint: () => 0,
    },
  };
}

describe("createWatcher (AC-16)", () => {
  test("first tick emits; unchanged tick returns null and re-parses nothing", () => {
    ledger("repoW1");
    const { src, treeCalls } = countingSource();
    const w = createWatcher(cfg, src);

    const first = w.tick(Date.now());
    expect(first).not.toBeNull();
    expect(first!.projects.length).toBeGreaterThanOrEqual(1);
    expect(treeCalls()).toBe(1); // selected project built once

    const idle = w.tick(Date.now());
    expect(idle).toBeNull(); // nothing changed
    expect(treeCalls()).toBe(1); // cache hit — no re-parse
  });

  test("mutating a ledger produces a fresh emit", () => {
    const p = ledger("repoW2");
    const { src } = countingSource();
    const w = createWatcher(cfg, src);
    w.tick(Date.now());
    expect(w.tick(Date.now())).toBeNull(); // settled

    // bump the ledger mtime deterministically (a real fs op)
    const future = new Date(Date.now() + 10_000);
    utimesSync(p, future, future);

    const after = w.tick(Date.now());
    expect(after).not.toBeNull();
  });

  test("changing the selection forces a rebuild", () => {
    ledger("repoW3");
    const { src, treeCalls } = countingSource();
    const w = createWatcher(cfg, src);
    const first = w.tick(Date.now())!;
    expect(w.tick(Date.now())).toBeNull();
    const before = treeCalls();

    // pick a concrete project id different from the default selection target
    const other = first.projects.find((p) => p.id !== first.selected?.id) ?? first.projects[0];
    w.setSelected(other.id);
    const after = w.tick(Date.now());
    expect(after).not.toBeNull();
    expect(treeCalls()).toBe(before + 1);
  });
});
