/**
 * PostToolUse push-waiver (#259), the native port of
 * `post-tools/modules/push-waiver.sh`: the push-review gate can only ask, and
 * a PostToolUse event for the push is the "yes". A successful, uninterrupted
 * push promotes the pending marker for exactly the diff that was asked about
 * (`pushWaiverPromote`); a failed push leaves it for the retry.
 *
 * Pushes are detected through `@toolu/core/shell` (#283 item 8: wrappers,
 * `bash -c`, `eval`, git by path and global options all count), and the
 * pushed repository is `@toolu/core/detect`'s `pushTargetRoot`. Success is
 * still the whole line's reported exit status, as in bash. It never writes
 * stdout.
 */
import type { Decision } from "../decision/decision.ts";
import type { ToolModule } from "../dispatch/dispatch-context.ts";
import { envValue } from "../host/host-name.ts";
import { pushWaiverPromote } from "../ledger/push-waiver.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { diffSha } from "../state/diff-sha.ts";
import { baseBranch, branchSlug } from "../detect/detect-branch.ts";
import { isGitPush, pushTargetRoot } from "../detect/detect-git.ts";
import { currentBranch } from "../state/state-git.ts";
import { commandAnalysis, isShellTool } from "./command-analysis.ts";
import { toolExitStatus, toolInterrupted } from "./tool-exit.ts";

const ALLOW: Decision = { kind: "allow" };

/** A reported exit status other than 0 means the push did not land. */
function pushFailed(status: string): boolean {
  return status !== "" && status !== "null" && status !== "0";
}

function decide(event: RegistryHookEvent, ctx: RegistryContext): Decision {
  if (!isShellTool(event)) return ALLOW;
  const analysis = commandAnalysis(event, ctx);
  if (!isGitPush(analysis)) return ALLOW;
  if (pushFailed(toolExitStatus(ctx.raw)) || toolInterrupted(ctx.raw)) return ALLOW;
  const cwd = ctx.cwd ?? process.cwd();
  const root = pushTargetRoot(analysis, { env: ctx.env, cwd });
  // Without git (bash: `command -v git || exit 0`) there is no branch either.
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
