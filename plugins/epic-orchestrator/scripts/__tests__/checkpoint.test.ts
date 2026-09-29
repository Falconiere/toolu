/** Worktree snapshots against real git repositories and linked worktrees. */

import { expect, test } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { snapshot, snapshotActive } from "../checkpoint.ts";

const IDENTITY = {
  GIT_AUTHOR_NAME: "t",
  GIT_AUTHOR_EMAIL: "t@t",
  GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@t",
};

async function git(cwd: string, ...args: string[]): Promise<string> {
  const res = await run(["git", "-C", cwd, ...args], { env: IDENTITY });
  if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

/** origin (bare) <- main clone, plus a linked worktree on a feature branch, all under `root`. */
async function setup(root: string): Promise<{ main: string; wt: string }> {
  const origin = join(root, "origin.git");
  const main = join(root, "main");
  await git(root, "init", "--bare", "-b", "main", origin);
  await git(root, "clone", origin, main);
  writeFileSync(join(main, "a.txt"), "one\n");
  writeFileSync(join(main, ".gitignore"), "node_modules/\n");
  await git(main, "add", ".");
  await git(main, "commit", "-m", "init");
  await git(main, "push", "origin", "main");
  const wt = join(root, "wt");
  await git(main, "worktree", "add", "-b", "feat/1-x", wt, "origin/main");
  return { main, wt };
}

test.concurrent("snapshot: clean, pushed worktree has nothing at risk", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  const s = await snapshot(wt, "k-1");
  expect(s.skipped).toBe("nothing at risk");
  expect(s.changed).toBe(false);
});

test.concurrent("snapshot: captures tracked, staged, and untracked work without touching the index", async () => {
  using sb = createSandbox();
  const { main, wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "two\n");
  writeFileSync(join(wt, "new.txt"), "untracked\n");
  writeFileSync(join(wt, "staged.txt"), "staged\n");
  await git(wt, "add", "staged.txt");
  const before = await git(wt, "status", "--porcelain");

  const s = await snapshot(wt, "k-1");
  expect(s.changed).toBe(true);
  expect(s.dirty).toBe(true);
  expect(await git(wt, "status", "--porcelain")).toBe(before);

  // The ref lives in the shared repo, so it outlives the worktree.
  await git(main, "worktree", "remove", "--force", wt);
  expect(await git(main, "show", "refs/epic-wip/k-1:a.txt")).toBe("two");
  expect(await git(main, "show", "refs/epic-wip/k-1:new.txt")).toBe("untracked");
  expect(await git(main, "show", "refs/epic-wip/k-1:staged.txt")).toBe("staged");
});

test.concurrent("snapshot: unchanged tree does not add a reflog entry; new work does", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "two\n");
  expect((await snapshot(wt, "k-2")).changed).toBe(true);
  expect((await snapshot(wt, "k-2")).changed).toBe(false);
  writeFileSync(join(wt, "a.txt"), "three\n");
  expect((await snapshot(wt, "k-2")).changed).toBe(true);
  const reflog = await git(wt, "reflog", "show", "--format=%H", "refs/epic-wip/k-2");
  expect(reflog.split("\n")).toHaveLength(2);
});

test.concurrent("snapshot: unpushed commits on a clean tree are pinned", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "committed\n");
  await git(wt, "commit", "-am", "wip");
  const s = await snapshot(wt, "k-3");
  expect(s.unpushed).toBe(1);
  expect(s.sha).toBe(await git(wt, "rev-parse", "HEAD"));
});

test.concurrent("snapshotActive only visits running issues unless a key is named", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "dirty\n");
  const state = join(sb.root, "state");
  mkdirSync(join(state, "issues"), { recursive: true });
  writeFileSync(
    join(state, "issues", "k-4.json"),
    JSON.stringify({ stage: "merged", worktree: wt }),
  );
  expect(await snapshotActive(state)).toEqual([]);
  const named = await snapshotActive(state, "k-4");
  expect(named[0]?.changed).toBe(true);
});
