/**
 * state-git (#255) against the bash it ports: `branch_slug` and
 * `detect_base_branch` from detect.sh, plus the unborn/detached HEAD shapes
 * the state libs read through `rev-parse --abbrev-ref HEAD`.
 */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { baseBranch, branchSlug, branchSlugs, currentBranch } from "../state-git.ts";

const DETECT_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/detect.sh");

async function bash(script: string, ...args: string[]): Promise<string> {
  const res = await run(["bash", "-c", `. "$1"; shift; ${script}`, "_", DETECT_SH, ...args]);
  expect(res.exitCode).toBe(0);
  return res.stdout.replace(/\n$/, "");
}

test.each([
  "feat/255-state",
  "release-please--branches--main",
  "fix/a.b@c",
  "////",
  "",
  "ünïcode/x",
  "a b",
])("branchSlug(%j) equals bash branch_slug", async (branch) => {
  expect(branchSlug(branch)).toBe(await bash('branch_slug "$1"', branch));
});

test("baseBranch follows origin/HEAD, else main, like detect_base_branch", async () => {
  using sb = createSandbox({ git: true, branch: "trunk" });
  expect(baseBranch(sb.project, process.env)).toBe("main");
  expect(await bash('detect_base_branch "$1"', sb.project)).toBe("main");
  sb.git("update-ref", "refs/remotes/origin/trunk", "HEAD");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk");
  expect(baseBranch(sb.project, process.env)).toBe("trunk");
  expect(await bash('detect_base_branch "$1"', sb.project)).toBe("trunk");
});

test("currentBranch: branch name, HEAD when detached or unborn, empty outside a repo", () => {
  using repo = createSandbox({ git: true, branch: "feat/x" });
  expect(currentBranch(repo.project, process.env)).toBe("feat/x");
  repo.git("checkout", "-q", "--detach");
  expect(currentBranch(repo.project, process.env)).toBe("HEAD");

  using unborn = createSandbox();
  unborn.git("init", "-q");
  expect(currentBranch(unborn.project, process.env)).toBe("HEAD");

  using plain = createSandbox();
  expect(currentBranch(plain.project, process.env)).toBe("");
});

test("branchSlugs lists local branches, and only merged ones with mergedInto", () => {
  using sb = createSandbox({ git: true });
  sb.git("branch", "feat/merged");
  sb.git("checkout", "-q", "-b", "feat/ahead");
  sb.write("a.txt", "a");
  sb.git("add", "a.txt");
  sb.git("commit", "-q", "-m", "ahead");
  expect([...branchSlugs(sb.project, process.env)].sort()).toEqual([
    "feat_ahead",
    "feat_merged",
    "main",
  ]);
  expect([...branchSlugs(sb.project, process.env, "main")].sort()).toEqual(["feat_merged", "main"]);
  // A base that does not exist merges nothing.
  expect(branchSlugs(sb.project, process.env, "nope").size).toBe(0);
});
