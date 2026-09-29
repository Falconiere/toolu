/** The few git calls the checks make, always run in the package directory. */
import { spawnSync } from "node:child_process";

export type GitResult = { status: number; stdout: string };

export function git(root: string, args: readonly string[]): GitResult {
  const res = spawnSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 });
  if (res.error) throw res.error;
  return { status: res.status ?? 1, stdout: res.stdout };
}

/** `git rev-parse --git-dir` succeeds: the directory is inside a work tree. */
export function inGitRepo(root: string): boolean {
  return git(root, ["rev-parse", "--git-dir"]).status === 0;
}
