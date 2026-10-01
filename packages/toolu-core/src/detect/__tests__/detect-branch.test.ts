/** Branch naming and base discovery in real git repositories. */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { baseBranch, branchSlug } from "../detect-branch.ts";

const SLUGS = ["feat/foo", "a/b/c", "feat#$%", "", "feat/255-state", "ünïcode/x", "a b", "////"];

test("branchSlug normalizes separators and strips punctuation", () => {
  expect(SLUGS.map(branchSlug)).toEqual([
    "feat_foo",
    "a_b_c",
    "feat",
    "_default",
    "feat_255-state",
    "ncode_x",
    "ab",
    "____",
  ]);
});

test("baseBranch uses origin/HEAD when present and main otherwise", () => {
  using sb = createSandbox({ git: true, branch: "trunk" });
  const outside = join(sb.root, "outside");
  mkdirSync(outside);
  const env = { HOME: sb.home, PATH: process.env.PATH ?? "/usr/bin:/bin" };

  const check = (expected: string) => {
    for (const [cwd, root] of [
      [sb.project, sb.project],
      [sb.project, ""],
      [outside, ""],
      [outside, sb.project],
    ] as const) {
      expect(baseBranch(root === "" ? undefined : root, env, cwd)).toBe(
        root === "" && cwd === outside ? "main" : expected,
      );
    }
  };
  check("main");
  sb.git("update-ref", "refs/remotes/origin/trunk", "HEAD");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk");
  check("trunk");
  expect(baseBranch(sb.project, env)).toBe("trunk");
  expect(baseBranch(undefined, env, outside)).toBe("main");
});
