/**
 * PostToolUse push-waiver (#259), the native port of
 * `post-tools/modules/push-waiver.sh`: the push-review gate can only ask, and
 * a PostToolUse event for the push is the "yes". A successful, uninterrupted
 * push promotes the pending marker for exactly the diff that was asked about
 * (`pushWaiverPromote`); a failed push leaves it for the retry.
 *
 * Pushes are detected through `@toolu/core/shell` (#283 item 8: wrappers,
 * `bash -c`, `eval`, git by path and global options all count). Success is
 * still the whole line's reported exit status, as in bash. It never writes
 * stdout.
 */
import { resolve } from "node:path";
import type { Decision } from "../decision/decision.ts";
import type { ToolModule } from "../dispatch/dispatch-context.ts";
import { envValue } from "../host/host-name.ts";
import { gitToplevel, projectRoot } from "../host/host-roots.ts";
import { pushWaiverPromote } from "../ledger/push-waiver.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { pushTargets, runsGitSubcommand } from "../shell/shell-git.ts";
import type { ShellAnalysis } from "../shell/shell-types.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch, branchSlug, currentBranch, hasGit } from "../state/state-git.ts";
import { commandAnalysis, isShellTool } from "./command-analysis.ts";
import { toolExitStatus, toolInterrupted } from "./tool-exit.ts";

const ALLOW: Decision = { kind: "allow" };

/** A reported exit status other than 0 means the push did not land. */
function pushFailed(status: string): boolean {
  return status !== "" && status !== "null" && status !== "0";
}

/**
 * `push_target_root`: the first push's `-C` chain replayed from the hook's
 * working directory (each value relative to the last, as git applies them),
 * else that directory's repository, the project root, the directory itself.
 * A dynamic `-C` value cannot be replayed.
 */
function pushRoot(analysis: ShellAnalysis, ctx: RegistryContext, cwd: string): string {
  const chain = pushTargets(analysis)[0]?.cChain ?? [];
  const dirs = chain.filter((dir) => dir !== null);
  const viaChain =
    dirs.length > 0 && dirs.length === chain.length
      ? gitToplevel(ctx.env, resolve(cwd, ...dirs))
      : undefined;
  return (
    viaChain ??
    gitToplevel(ctx.env, cwd) ??
    projectRoot({ env: ctx.env, host: ctx.host, cwd }) ??
    cwd
  );
}

function decide(event: RegistryHookEvent, ctx: RegistryContext): Decision {
  if (!isShellTool(event)) return ALLOW;
  const analysis = commandAnalysis(event, ctx);
  // Parse first: most calls are not pushes, and `hasGit` spawns a process.
  if (runsGitSubcommand(analysis, "push") !== "yes" || !hasGit(ctx.env)) return ALLOW;
  if (pushFailed(toolExitStatus(ctx.raw)) || toolInterrupted(ctx.raw)) return ALLOW;
  const cwd = ctx.cwd ?? process.cwd();
  const root = pushRoot(analysis, ctx, cwd);
  const branch = currentBranch(root, ctx.env);
  if (branch === "" || branch === "HEAD") return ALLOW;
  const base = envValue(ctx.env, "PUSH_REVIEW_BASE") ?? baseBranch(root, ctx.env);
  const sha = diffSha(root, base, { env: ctx.env });
  if (sha === undefined) return ALLOW;
  pushWaiverPromote(root, branchSlug(branch), sha, { env: ctx.env, host: ctx.host });
  return ALLOW;
}

/** The built-in `push-waiver.sh`, native. */
export const pushWaiverModule: ToolModule = {
  kind: "native",
  name: "push-waiver.sh",
  run: (event, ctx) => Promise.resolve(decide(event, ctx)),
};
