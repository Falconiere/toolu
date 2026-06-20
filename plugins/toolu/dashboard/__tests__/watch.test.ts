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
import { buildSummaries } from "../aggregate.ts";
import { discoverProjects } from "../discovery.ts";
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

  // Regression (default-target must match buildMultiState's active-filtered
  // selection): with no explicit selection, fingerprint()'s default target must
  // be buildSummaries(cfg, src)[0] — the newest-mtime *active* project — NOT the
  // newest-mtime project over ALL discovered projects. When the newest-mtime
  // project is IDLE (filtered out by isActive) while an older-mtime project is
  // ACTIVE (running step), buildMultiState renders ACTIVE but the OLD line
  // fingerprinted IDLE, so live-activity changes on the rendered ACTIVE project
  // were missed (tick returned null, no rebuild). This test fails on the OLD
  // line and passes on the fix.
  test("default target tracks the active project's activity, not the newest idle ledger", () => {
    // Isolate this scenario so other repos in `base` (all created ~now, hence
    // active under the default window) cannot win the default selection.
    const sub = mkdtempSync(join(base, "active-vs-idle-"));
    const scenarioCfg: DashboardConfig = {
      ...DEFAULT_CONFIG,
      roots: [sub],
      scanDepth: 2,
      // Collapse the time-based activity window to nothing (floored at 0 in
      // config), so a project is "active" ONLY via a running step. With this,
      // IDLE (empty steps, mtime in the past) is inactive regardless of mtime,
      // and ACTIVE is active purely because of its running step.
      activeWithinHours: 0,
    };

    // IDLE: empty ledger (no running step) -> inactive under the 0h window.
    const idleDir = join(sub, "IDLE", ".claude", "tmp", "plan-ledger");
    mkdirSync(idleDir, { recursive: true });
    const idlePath = join(idleDir, "main.json");
    writeFileSync(idlePath, JSON.stringify({ version: 1, branch: "main", base_branch: "main", plan_doc: "p.md", summary: {}, next: null, steps: [] }));

    // ACTIVE: ledger with one running step -> active via isActive's running gate.
    const activeDir = join(sub, "ACTIVE", ".claude", "tmp", "plan-ledger");
    mkdirSync(activeDir, { recursive: true });
    const activePath = join(activeDir, "main.json");
    writeFileSync(activePath, JSON.stringify({
      version: 1, branch: "main", base_branch: "main", plan_doc: "p.md", summary: {}, next: "s1",
      steps: [{ id: "s1", title: "go", check: "c", status: "running", started_at: null, activity: null, exit_code: null, diff_sha: null, last_run: null, evidence_tail: null }],
    }));

    // Make IDLE the NEWEST ledger mtime and ACTIVE strictly older — both in the
    // real past so IDLE stays inactive (buildSummaries uses real Date.now()).
    const realNow = Date.now();
    const idleMtime = new Date(realNow - 1000); // newest of the two, still past
    const activeMtime = new Date(realNow - 5000); // older than IDLE
    utimesSync(idlePath, idleMtime, idleMtime);
    utimesSync(activePath, activeMtime, activeMtime);

    // A per-project activity fingerprint backed by a counter we can bump, keyed
    // by project id — the same DI seam as countingSource (the real claude-code
    // adapter is covered elsewhere), not a hidden mock of selection logic.
    const activity = new Map<string, number>();
    const src: LiveActivitySource = {
      countSpawned: () => 0,
      tree: () => ({ agents: [] as AgentNode[], summary: { ...EMPTY } }),
      activityFingerprint: (project) => activity.get(project.id) ?? 0,
    };

    const w = createWatcher(scenarioCfg, src);

    // Resolve the deterministic project ids from discovery (sha1 of root@branch).
    const all = discoverProjects(scenarioCfg);
    const activeId = all.find((p) => p.root === join(sub, "ACTIVE"))!.id;
    const idleId = all.find((p) => p.root === join(sub, "IDLE"))!.id;
    expect(activeId).not.toBe(idleId);

    // Confirm the scenario against the REAL isActive/buildSummaries semantics:
    // IDLE (newest mtime) is filtered out; ACTIVE is the sole/first summary.
    const summaries = buildSummaries(scenarioCfg, src);
    expect(summaries.map((s) => s.id)).toEqual([activeId]);

    // 1) First tick emits and defaults selection to ACTIVE (matches buildMultiState).
    const fixedNow = realNow + 60_000;
    const first = w.tick(fixedNow);
    expect(first).not.toBeNull();
    expect(first!.selected).not.toBeNull();
    expect(first!.selected!.id).toBe(activeId);

    // Settle: nothing changed.
    expect(w.tick(fixedNow)).toBeNull();

    // 2) Bump ONLY ACTIVE's activity counter — no ledger mtime touched, IDLE's
    //    fingerprint unchanged. The change-gate must notice because the rendered
    //    (default) target is ACTIVE.
    activity.set(activeId, 1);

    // 3) Second tick must emit. With the OLD line the fingerprint tracked IDLE,
    //    whose activity is still 0, so this would (wrongly) return null.
    const after = w.tick(fixedNow);
    expect(after).not.toBeNull();
    expect(after!.selected!.id).toBe(activeId);
  });
});
