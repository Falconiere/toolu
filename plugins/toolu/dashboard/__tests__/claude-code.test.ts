// Real-data tests for the Claude Code activity adapter. Runs against the captured
// cc-store fixture (see fixtures/README.md) by pointing CLAUDE_CONFIG_DIR at it.
// Covers done/running/error/stale status, nesting, parse-free counting, slug
// parity with scan.sh, and graceful degradation. No mocks.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { join } from "node:path";

import type { DiscoveredProject } from "../discovery.ts";
import { claudeCodeSource, slugFor } from "../activity/claude-code.ts";

const STORE = join(import.meta.dir, "fixtures", "cc-store");
const SCAN_SH = join(import.meta.dir, "..", "..", "..", "stats", "scripts", "lib", "scan.sh");

const TU_A = "toolu_01CZjAm4dAWLvaZXKjiFvnCb"; // done parent
const TU_R = "toolu_0172xDeCHo4xPYFm2M8MGb4G"; // running
const TU_E = "toolu_01QhDS2hKGDLmgBWkztqdz9e"; // error

function project(root: string): DiscoveredProject {
  return { id: "t", root, branch: "x", label: "x", ledgerPath: "", ledgerMtimeMs: 0 };
}

const FIX = project("/fixture/repo");

let prevCfgDir: string | undefined;
beforeAll(() => {
  prevCfgDir = process.env.CLAUDE_CONFIG_DIR;
  process.env.CLAUDE_CONFIG_DIR = STORE;
});
afterAll(() => {
  if (prevCfgDir === undefined) delete process.env.CLAUDE_CONFIG_DIR;
  else process.env.CLAUDE_CONFIG_DIR = prevCfgDir;
});

describe("countSpawned (parse-free, AC-14 support)", () => {
  test("counts meta sidecars in the newest session", () => {
    expect(claudeCodeSource.countSpawned(FIX)).toBe(5);
  });
});

describe("tree", () => {
  // computed in beforeAll so CLAUDE_CONFIG_DIR (set by the outer beforeAll) is live
  let agents: ReturnType<typeof claudeCodeSource.tree>["agents"];
  let summary: ReturnType<typeof claudeCodeSource.tree>["summary"];
  let roots: Map<string, (typeof agents)[number]>;
  beforeAll(() => {
    const t = claudeCodeSource.tree(FIX, 0, 600);
    agents = t.agents;
    summary = t.summary;
    roots = new Map(agents.map((a) => [a.toolUseId, a]));
  });

  test("three top-level agents", () => {
    expect(agents.length).toBe(3);
    expect(roots.has(TU_A)).toBe(true);
    expect(roots.has(TU_R)).toBe(true);
    expect(roots.has(TU_E)).toBe(true);
  });

  test("AC-5: resolved agent is done with a non-negative duration", () => {
    const a = roots.get(TU_A)!;
    expect(a.status).toBe("done");
    expect(a.endedAt).not.toBeNull();
    expect(a.durationMs).not.toBeNull();
    expect(a.durationMs!).toBeGreaterThanOrEqual(0);
  });

  test("AC-5: errored result → error status", () => {
    expect(roots.get(TU_E)!.status).toBe("error");
  });

  test("AC-6: unresolved spawn → running, endedAt null", () => {
    const r = roots.get(TU_R)!;
    expect(r.status).toBe("running");
    expect(r.endedAt).toBeNull();
  });

  test("AC-8: nested children nest under their parent", () => {
    const a = roots.get(TU_A)!;
    expect(a.children.length).toBe(2);
    for (const child of a.children) {
      expect(child.parentToolUseId).toBe(TU_A);
      expect(child.status).toBe("done");
    }
    // a running top-level agent has no parent
    expect(roots.get(TU_R)!.parentToolUseId).toBeNull();
  });

  test("summary counts the whole tree", () => {
    expect(summary.total).toBe(5);
    expect(summary.running).toBe(1);
    expect(summary.errored).toBe(1);
    expect(summary.stale).toBe(0);
    expect(summary.deepest).toBe(2);
  });

  test("AC-7: an unpaired spawn older than agentStuckSeconds reads stale, not running", () => {
    const startedAt = roots.get(TU_R)!.startedAt!;
    const staleNow = Date.parse(startedAt) + 700_000; // 700s > 600s threshold
    const later = claudeCodeSource.tree(FIX, staleNow, 600);
    const r = later.agents.find((a) => a.toolUseId === TU_R)!;
    expect(r.status).toBe("stale");
    expect(later.summary.stale).toBe(1);
    expect(later.summary.running).toBe(0);
  });
});

describe("AC-9: slug parity with scan.sh", () => {
  function scanSlug(root: string): string {
    const out = execFileSync(
      "bash",
      ["-c", `source "${SCAN_SH}"; stats_cwd_to_slug "${root}"`],
      { encoding: "utf8" },
    );
    return out.trim();
  }

  test("TS slug matches scan.sh for real roots", () => {
    for (const root of ["/fixture/repo", "/Users/x/.herdr/worktrees/p/wt-1", "/a/b-c.d"]) {
      expect(slugFor(root)).toBe(scanSlug(root));
    }
  });
});

describe("AC-10: graceful degradation", () => {
  const NONE = project("/no/such/repo/anywhere");
  test("unknown project → 0 spawned, empty tree, zero summary, no throw", () => {
    expect(claudeCodeSource.countSpawned(NONE)).toBe(0);
    const { agents, summary } = claudeCodeSource.tree(NONE, Date.now(), 600);
    expect(agents).toEqual([]);
    expect(summary).toEqual({ total: 0, running: 0, errored: 0, stale: 0, deepest: 0 });
  });
});
