/**
 * Branch naming (#254): ports of `branch_slug` and `detect_base_branch`. The
 * state layer (#255) re-exports both from `state-git.ts`.
 */
import { spawnSync } from "node:child_process";
import { gitToplevel } from "../host/host-roots.ts";
import { childEnv, type HostEnv } from "../host/host-name.ts";

/** `branch_slug`: `/` becomes `_`, then everything outside `[A-Za-z0-9_-]` is dropped; empty is `_default`. */
export function branchSlug(branch: string): string {
  const slug = branch.replaceAll("/", "_").replace(/[^A-Za-z0-9_-]/g, "");
  return slug === "" ? "_default" : slug;
}

/**
 * `detect_base_branch [root]`: origin's HEAD branch, else `main`. Without a
 * root (or an empty one, bash `${1:-…}`) it resolves the git toplevel of
 * `cwd`, as bash does with `detect_project_root`; outside a repository it is
 * `main`.
 */
export function baseBranch(
  root: string | undefined,
  env: HostEnv = process.env,
  cwd?: string,
): string {
  const top = root === undefined || root === "" ? gitToplevel(env, cwd) : root;
  if (top === undefined) return "main";
  const res = spawnSync("git", ["-C", top, "symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"], {
    env: childEnv(env),
    encoding: "utf8",
  });
  const ref = res.error === undefined && res.status === 0 ? res.stdout.trim() : "";
  return ref === "" ? "main" : ref.replace(/^refs\/remotes\/origin\//, "");
}
