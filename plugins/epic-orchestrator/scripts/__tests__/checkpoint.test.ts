/** Worktree snapshots against real git repositories and linked worktrees. */

import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, readdirSync, unlinkSync, writeFileSync } from "node:fs";
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
  writeFileSync(join(main, "delete.txt"), "remove me\n");
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
  unlinkSync(join(wt, "delete.txt"));
  const before = await git(wt, "status", "--porcelain");
  const indexPath = await git(wt, "rev-parse", "--git-path", "index");
  const indexBefore = readFileSync(indexPath);

  const s = await snapshot(wt, "k-1");
  expect(s.changed).toBe(true);
  expect(s.dirty).toBe(true);
  expect(await git(wt, "status", "--porcelain")).toBe(before);
  expect(readFileSync(indexPath)).toEqual(indexBefore);

  // The ref lives in the shared repo, so it outlives the worktree.
  await git(main, "worktree", "remove", "--force", wt);
  expect(await git(main, "show", "refs/epic-wip/k-1:a.txt")).toBe("two");
  expect(await git(main, "show", "refs/epic-wip/k-1:new.txt")).toBe("untracked");
  expect(await git(main, "show", "refs/epic-wip/k-1:staged.txt")).toBe("staged");
  const deleted = await run(["git", "-C", main, "cat-file", "-e", "refs/epic-wip/k-1:delete.txt"]);
  expect(deleted.exitCode).not.toBe(0);
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

function tracedCommands(path: string): string[] {
  return readFileSync(path, "utf8")
    .trim()
    .split("\n")
    .flatMap((line) => {
      const value = JSON.parse(line) as Record<string, unknown>;
      if (value.event !== "start" || !Array.isArray(value.argv)) return [];
      return [value.argv.filter((part): part is string => typeof part === "string").join(" ")];
    });
}

test.concurrent("snapshot: retained index avoids redundant initialization and candidate commits", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  const trace = join(sb.root, "trace.json");
  writeFileSync(join(wt, "a.txt"), "dirty\n");
  const options = { env: { GIT_TRACE2_EVENT: trace } };

  expect((await snapshot(wt, "k-trace", options)).changed).toBe(true);
  expect((await snapshot(wt, "k-trace", options)).changed).toBe(false);

  const commands = tracedCommands(trace);
  expect(commands.filter((command) => command.endsWith(" read-tree HEAD"))).toHaveLength(1);
  expect(commands.filter((command) => command.includes(" commit-tree "))).toHaveLength(1);
  expect(commands.filter((command) => command.endsWith(" add -A"))).toHaveLength(2);
  expect(commands.filter((command) => command.endsWith(" write-tree"))).toHaveLength(2);
});

test.concurrent("snapshot: a new HEAD resets the retained index and snapshot ancestry", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  const trace = join(sb.root, "head-trace.json");
  const options = { env: { GIT_TRACE2_EVENT: trace } };
  writeFileSync(join(wt, "a.txt"), "first\n");
  expect((await snapshot(wt, "k-head", options)).changed).toBe(true);

  await git(wt, "add", "a.txt");
  await git(wt, "commit", "-m", "advance head");
  const head = await git(wt, "rev-parse", "HEAD");
  writeFileSync(join(wt, "new.txt"), "after head\n");
  expect((await snapshot(wt, "k-head", options)).changed).toBe(true);

  expect(await git(wt, "rev-parse", "refs/epic-wip/k-head^")).toBe(head);
  expect(await git(wt, "show", "refs/epic-wip/k-head:new.txt")).toBe("after head");
  const commands = tracedCommands(trace);
  expect(commands.filter((command) => command.endsWith(" read-tree HEAD"))).toHaveLength(2);
});

test.concurrent("snapshot: a corrupt retained index is reset from the same HEAD", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "dirty\n");
  expect((await snapshot(wt, "k-corrupt")).changed).toBe(true);
  const common = await git(wt, "rev-parse", "--path-format=absolute", "--git-common-dir");
  const root = join(common, "toolu", "checkpoint");
  const namespace = readdirSync(root)[0];
  expect(namespace).toBeDefined();
  if (namespace === undefined) throw new Error("missing retained checkpoint index");
  writeFileSync(join(root, namespace, "index"), "corrupt\n");

  const result = await snapshot(wt, "k-corrupt");
  expect(result.changed).toBe(false);
  expect(result.skipped).toBeUndefined();
});

test.concurrent("snapshot: a missing retained index is rebuilt before staging ignored tracked files", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, ".gitignore"), "node_modules/\na.txt\n");
  await git(wt, "add", ".gitignore");
  await git(wt, "commit", "-m", "ignore tracked file");
  writeFileSync(join(wt, "a.txt"), "first snapshot\n");
  expect((await snapshot(wt, "k-missing-index")).changed).toBe(true);

  const common = await git(wt, "rev-parse", "--path-format=absolute", "--git-common-dir");
  const root = join(common, "toolu", "checkpoint");
  const namespace = readdirSync(root)[0];
  if (namespace === undefined) throw new Error("missing retained checkpoint index");
  unlinkSync(join(root, namespace, "index"));
  writeFileSync(join(wt, "a.txt"), "after index deletion\n");

  expect((await snapshot(wt, "k-missing-index")).changed).toBe(true);
  expect(await git(wt, "show", "refs/epic-wip/k-missing-index:a.txt")).toBe("after index deletion");
});

test.concurrent("snapshot: concurrent attempts serialize one retained index", async () => {
  using sb = createSandbox();
  const { wt } = await setup(sb.root);
  writeFileSync(join(wt, "a.txt"), "dirty\n");
  const results = await Promise.all([snapshot(wt, "k-lock"), snapshot(wt, "k-lock")]);
  expect(results.filter((result) => result.changed)).toHaveLength(1);
  expect(
    results.every((result) => result.changed || result.skipped === "checkpoint busy" || result.sha),
  ).toBe(true);
  const reflog = await git(wt, "reflog", "show", "--format=%H", "refs/epic-wip/k-lock");
  expect(reflog.split("\n")).toHaveLength(1);
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
