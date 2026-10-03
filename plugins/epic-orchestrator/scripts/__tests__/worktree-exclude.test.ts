/** OpenCode worker state stays out of a linked worktree's status, snapshots and teardown. */

import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { snapshot } from "../checkpoint.ts";
import { OPENCODE_EXCLUDE, excludeOpencodeState } from "../opencode-worker.ts";

const IDENTITY = {
  GIT_AUTHOR_NAME: "t",
  GIT_AUTHOR_EMAIL: "t@t",
  GIT_COMMITTER_NAME: "t",
  GIT_COMMITTER_EMAIL: "t@t",
};
const MARKER = "# toolu epic-orchestrator: OpenCode worker runtime state";

async function git(cwd: string, ...args: string[]): Promise<string> {
  const res = await run(["git", "-C", cwd, ...args], { env: IDENTITY });
  if (res.exitCode !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trimEnd();
}

/** origin (bare) <- main clone, plus a linked worktree on a feature branch, all under `root`. */
async function setup(root: string): Promise<{ main: string; wt: string }> {
  const origin = join(root, "origin.git");
  const main = join(root, "main");
  await git(root, "init", "--bare", "-b", "main", origin);
  await git(root, "clone", origin, main);
  writeFileSync(join(main, "a.txt"), "one\n");
  await git(main, "add", ".");
  await git(main, "commit", "-m", "init");
  await git(main, "push", "origin", "main");
  const wt = join(root, "wt");
  await git(main, "worktree", "add", "-b", "feat/1-x", wt, "origin/main");
  return { main, wt };
}

function put(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

test.concurrent("excluded state leaves status, snapshot and worktree removal to the work", async () => {
  using sb = createSandbox();
  const { main, wt } = await setup(sb.root);
  rmSync(join(main, ".git", "info"), { recursive: true, force: true });
  put(join(wt, ".opencode/toolu/state/x"), "registry\n");
  put(join(wt, ".opencode/tmp/y"), "telemetry\n");
  put(join(wt, ".opencode/toolu/plugins.json"), '{"enabled":["epic-orchestrator"]}\n');
  writeFileSync(join(wt, "a.txt"), "two\n");

  const path = await excludeOpencodeState(wt);
  expect(await excludeOpencodeState(wt)).toBe(path);
  expect(path).toBe(join(main, ".git", "info", "exclude"));
  expect(readFileSync(path, "utf8")).toBe(`${[MARKER, ...OPENCODE_EXCLUDE].join("\n")}\n`);

  const status = await git(wt, "status", "--porcelain", "--untracked-files=all");
  expect(status.split("\n").toSorted()).toEqual([" M a.txt", "?? .opencode/toolu/plugins.json"]);

  const snap = await snapshot(wt, "k-1");
  expect(snap.changed).toBe(true);
  const tree = (await git(main, "ls-tree", "-r", "--name-only", snap.ref)).split("\n");
  expect(tree.toSorted()).toEqual([".opencode/toolu/plugins.json", "a.txt"]);
  expect(await git(main, "show", `${snap.ref}:a.txt`)).toBe("two");

  rmSync(join(wt, ".opencode/toolu/plugins.json"));
  await git(wt, "checkout", "--", "a.txt");
  await git(main, "worktree", "remove", wt);
  expect(existsSync(wt)).toBe(false);
});

test.concurrent("the exclude block is appended once, after a line without a newline", async () => {
  using sb = createSandbox();
  const { main, wt } = await setup(sb.root);
  const path = join(main, ".git", "info", "exclude");
  writeFileSync(path, `node_modules\n${MARKER}\n${OPENCODE_EXCLUDE[0]}\ncoverage`);
  await excludeOpencodeState(wt);
  await excludeOpencodeState(main);
  expect(readFileSync(path, "utf8")).toBe(
    `node_modules\n${MARKER}\n${OPENCODE_EXCLUDE[0]}\ncoverage\n${OPENCODE_EXCLUDE[1]}\n`,
  );
});

test.concurrent("a path outside any repository fails loudly", async () => {
  using sb = createSandbox();
  await expect(excludeOpencodeState(sb.root)).rejects.toThrow("rev-parse");
});
