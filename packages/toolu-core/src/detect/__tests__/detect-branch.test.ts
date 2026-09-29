/**
 * `branchSlug` and `baseBranch` against the unmodified `detect.sh` (#254 AC-3):
 * every `branch_slug` input its bats suite uses, and `detect_base_branch` with
 * and without `origin/HEAD`, with no argument, and outside a repository.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { bashBatch, cleanEnv } from "../../shell/__tests__/parity-helpers.ts";
import { baseBranch, branchSlug } from "../detect-branch.ts";

const SLUGS = ["feat/foo", "a/b/c", "feat#$%", "", "feat/255-state", "ünïcode/x", "a b", "////"];

test("branchSlug equals bash branch_slug on every bats input", () => {
  using sb = createSandbox();
  const env = cleanEnv(sb.home);
  const bash = bashBatch(
    "detect",
    'branch_slug "$f1"',
    SLUGS.map((s) => [s]),
    env,
  );
  expect(SLUGS.map(branchSlug)).toEqual(bash.map((out) => out.replace(/\n$/, "")));
  expect(branchSlug("")).toBe("_default");
});

test("baseBranch equals bash detect_base_branch with a root, no root, and outside git", () => {
  using sb = createSandbox({ git: true, branch: "trunk" });
  const outside = join(sb.root, "outside");
  mkdirSync(outside);
  const env = cleanEnv(sb.home);
  const ask = (cwd: string, root: string) =>
    (bashBatch("detect", 'detect_base_branch "$f1"', [[root]], env, cwd)[0] ?? "").trim();

  const check = (label: string) => {
    for (const [cwd, root] of [
      [sb.project, sb.project],
      [sb.project, ""],
      [outside, ""],
      [outside, sb.project],
    ] as const) {
      const ts = baseBranch(root === "" ? undefined : root, env, cwd);
      expect({ label, cwd, root, ts }).toEqual({ label, cwd, root, ts: ask(cwd, root) });
      expect(baseBranch(root, env, cwd)).toBe(ts);
    }
  };
  check("no origin/HEAD");
  sb.git("update-ref", "refs/remotes/origin/trunk", "HEAD");
  sb.git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/trunk");
  check("origin/HEAD -> trunk");
  expect(baseBranch(sb.project, env)).toBe("trunk");
  expect(baseBranch(undefined, env, outside)).toBe("main");
});
