// b1 check: buildState over REAL ledgers produced by the real plan-ledger.sh.
// No mocks and no hand-crafted ledger JSON — every fixture is a genuine git repo
// stamped by the engine, so stale/stuck are exercised against real diff shas.

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildState } from "../state.ts";

const ENGINE = join(
  import.meta.dir,
  "..",
  "..",
  "hooks",
  "lib",
  "plan-ledger.sh",
);

const PLAN = `# Tiny Plan — Plan

**Date:** 2026-06-17   **Status:** Approved   **Topic:** state.test fixture

## Steps (machine-readable)

\`\`\`json
[
  { "id": "s1", "title": "trivial true", "check": "true" }
]
\`\`\`
`;

/** Run a command in cwd, returning trimmed stdout; throws on non-zero. */
function run(cmd: string, args: string[], cwd: string): string {
  return execFileSync(cmd, args, { cwd, encoding: "utf8" }).trim();
}

/** Resolve the ledger path for a repo via the real engine (single truth). */
function ledgerPathOf(repo: string): string {
  return run("bash", [ENGINE, "path"], repo);
}

const tmps: string[] = [];
/** Create a throwaway temp dir, tracked for afterAll cleanup. */
function mkTmp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tmps.push(dir);
  return dir;
}

// A real git repo on a FEATURE branch (base=main) so main...HEAD is non-empty
// and a later commit changes the diff sha — the only way to exercise staleness.
let featureRepo: string;
let featureLedger: string;

beforeAll(() => {
  featureRepo = mkTmp("pl-state-feat-");
  run("git", ["init", "-q", "-b", "main"], featureRepo);
  run("git", ["config", "user.email", "t@t.t"], featureRepo);
  run("git", ["config", "user.name", "t"], featureRepo);
  writeFileSync(join(featureRepo, "file.txt"), "base\n");
  run("git", ["add", "-A"], featureRepo);
  run("git", ["commit", "-qm", "init"], featureRepo);
  run("git", ["checkout", "-q", "-b", "feature"], featureRepo);
  writeFileSync(join(featureRepo, "plan.md"), PLAN);
  // Stamp s1 green via the REAL engine — its diff_sha is the current main...HEAD.
  run("bash", [ENGINE, "run", "plan.md", "--step", "s1"], featureRepo);
  featureLedger = ledgerPathOf(featureRepo);
});

afterAll(() => {
  for (const dir of tmps) rmSync(dir, { recursive: true, force: true });
});

describe("buildState — real ledger fixtures", () => {
  test("a fresh green step reads stale:false", () => {
    const state = buildState({ ledgerPath: featureLedger, repoRoot: featureRepo });
    expect(state.ledger).not.toBeNull();
    expect(state.currentDiffSha).not.toBeNull();
    const s1 = state.steps.find((s) => s.id === "s1");
    expect(s1?.status).toBe("green");
    expect(s1?.stale).toBe(false);
    expect(s1?.stuck).toBe(false);
  });

  test("a new commit (changes base...HEAD) makes the prior green step stale", () => {
    // New commit on feature -> main...HEAD sha changes, stored diff_sha does not.
    writeFileSync(join(featureRepo, "file.txt"), "base\nmore\n");
    run("git", ["add", "-A"], featureRepo);
    run("git", ["commit", "-qm", "more work"], featureRepo);
    const state = buildState({ ledgerPath: featureLedger, repoRoot: featureRepo });
    const s1 = state.steps.find((s) => s.id === "s1");
    expect(s1?.status).toBe("green");
    expect(s1?.stale).toBe(true);
  });

  test("same result when the process cwd is outside the repo (git -C <root>)", () => {
    const outside = mkTmp("pl-state-outside-");
    const prevCwd = process.cwd();
    try {
      process.chdir(outside);
      const state = buildState({ ledgerPath: featureLedger, repoRoot: featureRepo });
      const s1 = state.steps.find((s) => s.id === "s1");
      // Identical to the in-repo result above: green + stale after the commit.
      expect(s1?.status).toBe("green");
      expect(s1?.stale).toBe(true);
      expect(state.currentDiffSha).not.toBeNull();
    } finally {
      process.chdir(prevCwd);
    }
  });

  test("absent ledger -> {ledger:null, currentDiffSha:null, steps:[]}", () => {
    const empty = mkTmp("pl-state-noledger-");
    const state = buildState({
      ledgerPath: join(empty, "missing.json"),
      repoRoot: featureRepo,
    });
    expect(state.ledger).toBeNull();
    expect(state.currentDiffSha).toBeNull();
    expect(state.steps).toEqual([]);
    expect(state.error).toBeUndefined();
  });

  test("non-git repoRoot -> currentDiffSha:null and every step stale:false", () => {
    const nonGit = mkTmp("pl-state-nongit-");
    // Reuse the real feature ledger (has a green step) but point git at a non-repo.
    const state = buildState({ ledgerPath: featureLedger, repoRoot: nonGit });
    expect(state.currentDiffSha).toBeNull();
    expect(state.steps.length).toBeGreaterThan(0);
    expect(state.steps.every((s) => s.stale === false)).toBe(true);
  });

  test("corrupt ledger json -> error-shaped payload, no throw", () => {
    const corruptDir = mkTmp("pl-state-corrupt-");
    const corrupt = join(corruptDir, "ledger.json");
    writeFileSync(corrupt, "{ not valid json ]");
    const state = buildState({ ledgerPath: corrupt, repoRoot: featureRepo });
    expect(state.ledger).toBeNull();
    expect(state.steps).toEqual([]);
    expect(typeof state.error).toBe("string");
  });

  test("a real running step past the stuck threshold reads stuck:true", async () => {
    // Background a --step run whose check sleeps, so the ledger genuinely holds a
    // running step with a real started_at; poll until write #1 lands, then a 0s
    // threshold proves the stuck derivation against real engine state.
    const repo = mkTmp("pl-state-running-");
    run("git", ["init", "-q", "-b", "main"], repo);
    run("git", ["config", "user.email", "t@t.t"], repo);
    run("git", ["config", "user.name", "t"], repo);
    writeFileSync(join(repo, "file.txt"), "x\n");
    run("git", ["add", "-A"], repo);
    run("git", ["commit", "-qm", "init"], repo);
    writeFileSync(
      join(repo, "plan.md"),
      PLAN.replace('"check": "true"', '"check": "sleep 1; true"'),
    );
    const ledger = ledgerPathOf(repo);
    // Truly detached so the test process does not block on the 1s sleep.
    const child = Bun.spawn(["bash", ENGINE, "run", "plan.md", "--step", "s1"], {
      cwd: repo,
      stdout: "ignore",
      stderr: "ignore",
    });
    // Poll the on-disk ledger for the `running` write #1 the engine emits before
    // the check runs (AC#2 mid-run observability), with a generous deadline.
    let s1;
    const deadline = Date.now() + 2000;
    while (Date.now() < deadline) {
      if (existsSync(ledger)) {
        const state = buildState({ ledgerPath: ledger, repoRoot: repo, stuckThresholdSeconds: 0 });
        s1 = state.steps.find((s) => s.id === "s1");
        if (s1?.status === "running") break;
      }
      await Bun.sleep(20);
    }
    expect(s1?.status).toBe("running");
    expect(s1?.started_at).not.toBeNull();
    expect(s1?.stuck).toBe(true);
    await child.exited;
  });
});
