/**
 * Git operations a command performs (#254): `is_git_push`, `is_git_commit`,
 * `push_target_root` and `push_target_branch`, answered from the command's
 * `ShellAnalysis` (`@toolu/core/shell`, #284) rather than its text. A gate
 * parses once per event (`shellAnalysisOf`) and passes the analysis in, so
 * this module never loads the parser. Fixes #283 items 8 and 9.
 */
import { spawnSync } from "node:child_process";
import { childEnv, type HostEnv } from "../host/host-name.ts";
import { projectRoot } from "../host/host-roots.ts";
import { pushTargets, runsGitSubcommand } from "../shell/shell-git.ts";
import type { ShellAnalysis } from "../shell/shell-types.ts";
import type { DetectOptions } from "./detect-project.ts";

/**
 * Whether the command runs `git push`. A dynamic name (`$g push`) is
 * `unknown` to `runsGitSubcommand` and `false` here, as bash answers for the
 * workflow gates; a gate that must fail closed asks `runsGitSubcommand`.
 */
export function isGitPush(analysis: ShellAnalysis): boolean {
  return runsGitSubcommand(analysis, "push") === "yes";
}

/** Whether the command runs `git commit`; `unknown` is `false`, as for `isGitPush`. */
export function isGitCommit(analysis: ShellAnalysis): boolean {
  return runsGitSubcommand(analysis, "commit") === "yes";
}

/** Trimmed stdout of `git args` in `cwd`, or `undefined` when git fails or prints nothing. */
function gitOut(
  cwd: string | undefined,
  args: readonly string[],
  env: HostEnv,
): string | undefined {
  const res = spawnSync("git", [...args], {
    cwd: cwd ?? process.cwd(),
    env: childEnv(env),
    encoding: "utf8",
  });
  const out = res.error === undefined && res.status === 0 ? res.stdout.trim() : "";
  return out === "" ? undefined : out;
}

/**
 * `push_target_root`: the toplevel of the repository the first push targets.
 * Its whole `-C` chain is replayed, since git applies each relative to the
 * last. A chain with a dynamic value, or one git cannot resolve, falls back to
 * the cwd's toplevel, then the host project root, then the cwd.
 */
export function pushTargetRoot(analysis: ShellAnalysis, options: DetectOptions = {}): string {
  const env = options.env ?? process.env;
  const cwd = options.cwd ?? process.cwd();
  const chain = pushTargets(analysis)[0]?.cChain ?? [];
  const dirs = chain.filter((dir) => dir !== null);
  const viaChain =
    chain.length > 0 && dirs.length === chain.length
      ? gitOut(cwd, [...dirs.flatMap((dir) => ["-C", dir]), "rev-parse", "--show-toplevel"], env)
      : undefined;
  return (
    viaChain ??
    gitOut(cwd, ["rev-parse", "--show-toplevel"], env) ??
    projectRoot({ env, cwd }) ??
    cwd
  );
}

/**
 * `push_target_branch`: the checked-out branch of `root`; on a detached HEAD,
 * the branch the first push's refspec names (`HEAD:x`, `+src:refs/heads/x`,
 * bare `x`). `""` when none can be named: a delete, bare `HEAD`, a wildcard,
 * no refspec, or a dynamic one.
 */
export function pushTargetBranch(
  analysis: ShellAnalysis,
  root: string,
  env: HostEnv = process.env,
): string {
  const branch = gitOut(root, ["rev-parse", "--abbrev-ref", "HEAD"], env);
  if (branch !== undefined && branch !== "HEAD") return branch;
  return pushTargets(analysis)[0]?.destination ?? "";
}
