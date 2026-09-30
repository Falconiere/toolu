/**
 * The push the workflow gates judge (#262): push-review, plan-ledger and
 * docs-sync act only on a Bash/Shell call that runs `git push`, found in the
 * parsed command rather than its text (#283 item 8), and judge the repository
 * and branch that push targets: the whole `-C` chain, then the refspec
 * destination on a detached HEAD (#283 item 9). A line the parser could not
 * analyze is not a push to them: none is a security guardrail, and treating
 * every oversize command as a push would nag on each large heredoc.
 */
import { spawnSync } from "node:child_process";
import { isGitPush, pushTargetBranch, pushTargetRoot } from "../detect/detect-git.ts";
import { toolAvailable } from "../detect/detect-tools.ts";
import { childEnv, type HostEnv } from "../host/host-name.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { shellAnalysisOf } from "../shell/shell-event.ts";

export type PushTarget = {
  /** The toplevel of the pushed repository. */
  readonly root: string;
  /** Its checked-out branch, else the refspec destination; `""` when neither names one. */
  readonly branch: string;
};

/** The push `event` runs, or undefined when it runs none (or git is not installed). */
export function pushTarget(event: RegistryHookEvent, ctx: RegistryContext): PushTarget | undefined {
  if (event.type !== "shell/pre") return undefined;
  const analysis = shellAnalysisOf(event);
  if (!isGitPush(analysis) || !toolAvailable("git", ctx.env)) return undefined;
  const root = pushTargetRoot(analysis, { env: ctx.env, cwd: ctx.cwd ?? process.cwd() });
  return { root, branch: pushTargetBranch(analysis, root, ctx.env) };
}

/** `git -C ROOT ARGS...` stdout, or undefined on a non-zero exit. */
export function gitAt(root: string, args: readonly string[], env: HostEnv): string | undefined {
  const res = spawnSync("git", ["-C", root, ...args], { env: childEnv(env), encoding: "utf8" });
  return res.error === undefined && res.status === 0 ? res.stdout : undefined;
}

/** `git rev-parse --verify --quiet REF` in `root`. */
export function refExists(root: string, ref: string, env: HostEnv): boolean {
  return gitAt(root, ["rev-parse", "--verify", "--quiet", ref], env) !== undefined;
}

/** `git diff --no-color BASE...HEAD --name-only` in `root`, one path per line; `""` on failure. */
export function changedNames(root: string, base: string, env: HostEnv): string {
  return gitAt(root, ["diff", "--no-color", `${base}...HEAD`, "--name-only"], env) ?? "";
}
