/** Verified worker start, retention, replacement, and exact-session resume. */

import { CommandError, herdr } from "../common.ts";
import { agentArgs, hostKind, type HostKind } from "../hosts.ts";
import { inspectAgent, requireIdentity, shutdownAgent } from "../lifecycle.ts";
import { requireCapturedSession } from "../session.ts";
import { shellJoin } from "../launch-worktree.ts";

type AgentOptions = { dryRun: boolean; replace: boolean };

export type AgentPlan = {
  host: { kind: HostKind; model?: string | undefined; effort?: string | undefined };
  bypass: boolean;
  permissionMode: string;
  previousKind: string | undefined;
  launches: number;
  worktree: string;
  sessionId?: string;
  sessionStartedAfter?: number;
};

async function requireRecordedSession(kind: HostKind, plan: AgentPlan): Promise<void> {
  if (!plan.sessionId || plan.sessionStartedAfter === undefined)
    throw new CommandError("live session identity is missing; inspect before explicit --replace");
  try {
    await requireCapturedSession(kind, plan.worktree, plan.sessionStartedAfter, plan.sessionId);
  } catch {
    throw new CommandError(
      "live session identity is missing or changed; inspect before explicit --replace",
    );
  }
}

async function stopAgent(key: string, log: string[], plan: AgentPlan, pane: string): Promise<void> {
  log.push(`herdr agent prompt ${key} /exit`);
  const outcome = await shutdownAgent(key, {
    kind: hostKind(plan.previousKind ?? plan.host.kind),
    pane,
    cwd: plan.worktree,
  });
  if (outcome !== "exited")
    throw new CommandError(`agent ${key} did not exit; reconcile before --replace`);
}

/** Start or retain the worker. Returns [started, resumed]. */
export async function ensureAgent(
  key: string,
  pane: string,
  plan: AgentPlan,
  opts: AgentOptions,
  log: string[],
): Promise<[boolean, boolean]> {
  const { kind } = plan.host;
  if (!opts.dryRun) {
    const live = await inspectAgent(key);
    if (live) {
      requireIdentity(live, {
        kind: hostKind(plan.previousKind ?? kind),
        pane,
        cwd: plan.worktree,
      });
      const moving = plan.previousKind !== undefined && plan.previousKind !== kind;
      if (moving && !opts.replace)
        throw new CommandError("host change requires explicit --replace");
      if (!opts.replace) {
        await requireRecordedSession(kind, plan);
        log.push(`agent ${key} already live`);
        return [false, false];
      }
      await stopAgent(key, log, plan, pane);
    }
  }
  const resume = plan.launches > 0 && plan.previousKind === kind && !opts.replace;
  if (resume && !plan.sessionId)
    throw new CommandError(
      "captured session ID missing; reconcile the previous launch or explicitly --replace",
    );
  const args = agentArgs(kind, {
    key,
    model: plan.host.model,
    effort: plan.host.effort,
    bypass: plan.bypass,
    permissionMode: plan.permissionMode,
    resume,
    ...(plan.sessionId ? { sessionId: plan.sessionId } : {}),
  });
  const cmd = [
    "agent",
    "start",
    key,
    "--kind",
    kind,
    "--pane",
    pane,
    "--timeout",
    "90000",
    "--",
    ...args,
  ];
  log.push("herdr " + shellJoin(cmd));
  if (!opts.dryRun) await herdr(cmd);
  if (resume && !opts.dryRun) {
    const live = await inspectAgent(key);
    if (!live) throw new CommandError("resumed agent disappeared before identity verification");
    requireIdentity(live, { kind, pane, cwd: plan.worktree });
    await requireRecordedSession(kind, plan);
  }
  return [true, resume];
}
