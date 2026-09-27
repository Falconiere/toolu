/** Worktree snapshots against real git repositories and linked worktrees. */

import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { snapshot, snapshotActive } from "../checkpoint.ts";

function git(cwd: string, ...args: string[]): string {
  const p = Bun.spawnSync(["git", "-C", cwd, ...args], {
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: "t",
      GIT_AUTHOR_EMAIL: "t@t",
      GIT_COMMITTER_NAME: "t",
      GIT_COMMITTER_EMAIL: "t@t",
    },
  });
  if (p.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${String(p.stderr)}`);
  return p.stdout.toString().trim();
}

/** origin (bare) <- main clone, plus a linked worktree on a feature branch. */
function setup(): { main: string; wt: string } {
  const root = mkdtempSync(join(tmpdir(), "epic-ckpt-"));
  const origin = join(root, "origin.git");
  const main = join(root, "main");
  git(root, "init", "--bare", "-b", "main", origin);
  git(root, "clone", origin, main);
  writeFileSync(join(main, "a.txt"), "one\n");
  writeFileSync(join(main, ".gitignore"), "node_modules/\n");
  git(main, "add", ".");
  git(main, "commit", "-m", "init");
  git(main, "push", "origin", "main");
  const wt = join(root, "wt");
  git(main, "worktree", "add", "-b", "feat/1-x", wt, "origin/main");
  return { main, wt };
}

describe("snapshot", () => {
  test("clean, pushed worktree has nothing at risk", async () => {
    const { wt } = setup();
    const s = await snapshot(wt, "k-1");
    expect(s.skipped).toBe("nothing at risk");
    expect(s.changed).toBe(false);
  });

  test("captures tracked, staged, and untracked work without touching the index", async () => {
    const { main, wt } = setup();
    writeFileSync(join(wt, "a.txt"), "two\n");
    writeFileSync(join(wt, "new.txt"), "untracked\n");
    writeFileSync(join(wt, "staged.txt"), "staged\n");
    git(wt, "add", "staged.txt");
    const before = git(wt, "status", "--porcelain");

    const s = await snapshot(wt, "k-1");
    expect(s.changed).toBe(true);
    expect(s.dirty).toBe(true);
    expect(git(wt, "status", "--porcelain")).toBe(before);

    // The ref lives in the shared repo, so it outlives the worktree.
    git(main, "worktree", "remove", "--force", wt);
    expect(git(main, "show", "refs/epic-wip/k-1:a.txt")).toBe("two");
    expect(git(main, "show", "refs/epic-wip/k-1:new.txt")).toBe("untracked");
    expect(git(main, "show", "refs/epic-wip/k-1:staged.txt")).toBe("staged");
  });

  test("unchanged tree does not add a reflog entry; new work does", async () => {
    const { wt } = setup();
    writeFileSync(join(wt, "a.txt"), "two\n");
    expect((await snapshot(wt, "k-2")).changed).toBe(true);
    expect((await snapshot(wt, "k-2")).changed).toBe(false);
    writeFileSync(join(wt, "a.txt"), "three\n");
    expect((await snapshot(wt, "k-2")).changed).toBe(true);
    expect(git(wt, "reflog", "show", "--format=%H", "refs/epic-wip/k-2").split("\n")).toHaveLength(
      2,
    );
  });

  test("unpushed commits on a clean tree are pinned", async () => {
    const { wt } = setup();
    writeFileSync(join(wt, "a.txt"), "committed\n");
    git(wt, "commit", "-am", "wip");
    const s = await snapshot(wt, "k-3");
    expect(s.unpushed).toBe(1);
    expect(s.sha).toBe(git(wt, "rev-parse", "HEAD"));
  });
});

test("snapshotActive only visits running issues unless a key is named", async () => {
  const { wt } = setup();
  writeFileSync(join(wt, "a.txt"), "dirty\n");
  const state = mkdtempSync(join(tmpdir(), "epic-state-"));
  mkdirSync(join(state, "issues"));
  writeFileSync(
    join(state, "issues", "k-4.json"),
    JSON.stringify({ stage: "merged", worktree: wt }),
  );
  expect(await snapshotActive(state)).toEqual([]);
  const named = await snapshotActive(state, "k-4");
  expect(named[0]?.changed).toBe(true);
});
