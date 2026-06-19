// Real-data tests for discovery: builds an actual temp directory tree with real
// plan-ledger JSON files at various depths, then walks it — no mocks.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { DEFAULT_CONFIG, type DashboardConfig } from "../config.ts";
import { discoverProjects } from "../discovery.ts";

const base = mkdtempSync(join(tmpdir(), "toolu-dash-discovery-"));
afterAll(() => rmSync(base, { recursive: true, force: true }));

/** Write a real ledger file at <root>/.claude/tmp/plan-ledger/<slug>.json. */
function ledger(root: string, slug: string, branch: string): string {
  const dir = join(root, ".claude", "tmp", "plan-ledger");
  mkdirSync(dir, { recursive: true });
  const p = join(dir, `${slug}.json`);
  writeFileSync(
    p,
    JSON.stringify({ version: 1, branch, base_branch: "main", plan_doc: "x.md", steps: [] }),
  );
  return p;
}

let pathA: string;
let pathB: string;
let pathDeep: string;

beforeAll(() => {
  pathA = ledger(join(base, "repoA"), "main", "main");
  pathB = ledger(join(base, "repoB"), "feat_x", "feat/x");
  // depth 3 below base: base/nested/sub/repoC
  pathDeep = ledger(join(base, "nested", "sub", "repoC"), "main", "main");
  // pruned: ledgers under .git and node_modules must never be returned
  ledger(join(base, ".git", "worktrees", "ghost"), "main", "ghost");
  ledger(join(base, "node_modules", "pkg"), "main", "dep");
});

function cfg(overrides: Partial<DashboardConfig>): DashboardConfig {
  return { ...DEFAULT_CONFIG, roots: [base], ...overrides };
}

describe("discoverProjects", () => {
  test("AC-1: finds both top-level repos with correct root/branch/ledgerPath", () => {
    const found = discoverProjects(cfg({ scanDepth: 2 }));
    const byPath = new Map(found.map((p) => [p.ledgerPath, p]));
    expect(byPath.has(pathA)).toBe(true);
    expect(byPath.has(pathB)).toBe(true);
    expect(byPath.get(pathA)!.root).toBe(join(base, "repoA"));
    expect(byPath.get(pathA)!.branch).toBe("main");
    expect(byPath.get(pathB)!.branch).toBe("feat/x"); // read from ledger, not the slug
    expect(byPath.get(pathB)!.label).toBe("repoB · feat/x");
  });

  test("AC-1: prunes .git and node_modules", () => {
    const found = discoverProjects(cfg({ scanDepth: 5 }));
    const branches = found.map((p) => p.branch);
    expect(branches).not.toContain("ghost");
    expect(branches).not.toContain("dep");
  });

  test("AC-2: a ledger deeper than scanDepth is not returned", () => {
    const shallow = discoverProjects(cfg({ scanDepth: 2 }));
    expect(shallow.map((p) => p.ledgerPath)).not.toContain(pathDeep);
    const deep = discoverProjects(cfg({ scanDepth: 3 }));
    expect(deep.map((p) => p.ledgerPath)).toContain(pathDeep);
  });

  test("nonexistent roots are skipped (no throw)", () => {
    const found = discoverProjects(cfg({ roots: [join(base, "nope"), base], scanDepth: 2 }));
    expect(found.length).toBeGreaterThanOrEqual(2);
  });

  test("ids are stable and unique per (root, branchSlug)", () => {
    const a = discoverProjects(cfg({ scanDepth: 3 }));
    const b = discoverProjects(cfg({ scanDepth: 3 }));
    const ids = a.map((p) => p.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(a.find((p) => p.ledgerPath === pathA)!.id).toBe(
      b.find((p) => p.ledgerPath === pathA)!.id,
    );
  });

  test("overlapping roots dedupe by ledger path", () => {
    const found = discoverProjects(cfg({ roots: [base, join(base, "repoA")], scanDepth: 3 }));
    const a = found.filter((p) => p.ledgerPath === pathA);
    expect(a.length).toBe(1);
  });
});
