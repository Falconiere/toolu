/**
 * The git questions the state layer asks (#255): the `rev-parse`/`branch`
 * calls in the bash state libs. `branchSlug` and `baseBranch` live in the
 * detect layer (#254) and are re-exported here for the state callers.
 */
import { spawnSync } from "node:child_process";
import { branchSlug } from "../detect/detect-branch.ts";
import { childEnv, type HostEnv } from "../host/host-name.ts";

export { baseBranch, branchSlug } from "../detect/detect-branch.ts";

/** Stdout of `git -C root args`, or undefined when git cannot run or exits non-zero. */
function git(root: string, args: readonly string[], env: HostEnv): string | undefined {
  const res = spawnSync("git", ["-C", root, ...args], { env: childEnv(env), encoding: "utf8" });
  return res.error === undefined && res.status === 0 ? res.stdout : undefined;
}

/** `git --version` runs: bash's `command -v git`. */
export function hasGit(env: HostEnv): boolean {
  const res = spawnSync("git", ["--version"], { env: childEnv(env), encoding: "utf8" });
  return res.error === undefined && res.status === 0;
}

/**
 * `$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")`. Returns "HEAD"
 * when HEAD is detached or unborn (git prints it, then exits 128), and "" when
 * `root` is not a repository. Callers treat both as no branch.
 */
export function currentBranch(root: string, env: HostEnv): string {
  const res = spawnSync("git", ["-C", root, "rev-parse", "--abbrev-ref", "HEAD"], {
    env: childEnv(env),
    encoding: "utf8",
  });
  return res.error === undefined ? res.stdout.replace(/\n+$/, "") : "";
}

/** Slugs of local branches, optionally only those merged into `mergedInto`. */
export function branchSlugs(root: string, env: HostEnv, mergedInto?: string): Set<string> {
  const args = ["branch", "--format=%(refname:short)"];
  if (mergedInto !== undefined) args.push("--merged", mergedInto);
  const out = git(root, args, env) ?? "";
  return new Set(
    out
      .split("\n")
      .filter((name) => name !== "")
      .map(branchSlug),
  );
}
