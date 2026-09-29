/**
 * The git questions the state layer asks (#255). These are private ports of
 * `branch_slug`, `detect_base_branch` and the `rev-parse`/`branch` calls in
 * the bash state libs. #254 owns the public detect layer.
 */
import { spawnSync } from "node:child_process";
import { childEnv, type HostEnv } from "../host/host-name.ts";

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

/** `branch_slug`: `/` becomes `_`, then everything outside `[A-Za-z0-9_-]` is dropped; empty is `_default`. */
export function branchSlug(branch: string): string {
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  return slug === "" ? "_default" : slug;
}

/** `detect_base_branch`: origin's HEAD branch, else `main`. */
export function baseBranch(root: string, env: HostEnv): string {
  const ref = git(root, ["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"], env)?.trim();
  return ref === undefined || ref === "" ? "main" : ref.replace(/^refs\/remotes\/origin\//, "");
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
