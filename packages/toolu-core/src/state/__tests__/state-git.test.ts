/** Branch state in real repositories, including detached and unborn HEAD. */
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { baseBranch, branchSlug, branchSlugs, currentBranch } from "../state-git.ts";

test.each([
  ["feat/255-state", "feat_255-state"],
  ["release-please--branches--main", "release-please--branches--main"],
  ["fix/a.b@c", "fix_abc"],
  ["////", "____"],
  ["", "_default"],
  ["ünïcode/x", "ncode_x"],
  ["a b", "ab"],
])("branchSlug(%j) returns %j", (branch, expected) => {
  expect(branchSlug(branch)).toBe(expected);
});

test("baseBranch follows origin/HEAD, else main", () => {
  using sb = createSandbox({ git: true, branch: "trunk" });
  expect(baseBranch(sb.project, process.env)).toBe("main");
  sb.git("update-ref", "refs/remotes/origin/trunk", "HEAD");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk");
  expect(baseBranch(sb.project, process.env)).toBe("trunk");
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
