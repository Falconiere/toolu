/** Typed Herdr observations; failed queries never mean a worker has exited. */
import { realpathSync } from "node:fs";
import { CommandError, herdr } from "./common.ts";
import { HOST_LIMIT, hostKind, type HostKind } from "./hosts.ts";
import { runCommand } from "../hooks/dist/epic-runtime.js";
import { ownedWorkload, verifyOwnedExit } from "./workload.ts";

export type LifecycleOutcome =
  | "started"
  | "resumed"
  | "retained"
  | "uncertain"
  | "blocked"
  | "provider-limited"
  | "exited"
  | "cleanup-incomplete";
export type LiveAgent = { name: string; kind: HostKind; pane: string; cwd: string; status: string };

export async function inspectAgent(name: string): Promise<LiveAgent | null> {
  const result = await herdr(["agent", "list"], 10_000);
  if (!Array.isArray(result.agents)) throw new CommandError("herdr agent list omitted agents");
  const agent = result.agents.find((a: { name?: string }) => a.name === name) as
    | Record<string, unknown>
    | undefined;
  if (!agent) return null;
  if (
    typeof agent.agent !== "string" ||
    typeof agent.pane_id !== "string" ||
    typeof agent.cwd !== "string"
  )
    throw new CommandError(`cannot verify live identity of ${name}`);
  return {
    name,
    kind: hostKind(agent.agent),
    pane: agent.pane_id,
    cwd: agent.cwd,
    status: typeof agent.agent_status === "string" ? agent.agent_status : "unknown",
  };
}

export function requireIdentity(
  live: LiveAgent,
  expected: { kind: HostKind; pane: string; cwd: string },
): void {
  if (
    live.kind !== expected.kind ||
    live.pane !== expected.pane ||
    realpathSync(live.cwd) !== realpathSync(expected.cwd)
  )
    throw new CommandError(
      `live agent ${live.name} identity does not match recorded host/pane/worktree`,
    );
}

export function acknowledgedStatus(
  status: string,
  tail: string,
  fallback: LifecycleOutcome,
): LifecycleOutcome {
  if (HOST_LIMIT.test(tail)) return "provider-limited";
  return status === "blocked" ? "blocked" : fallback;
}

/** A prompt acknowledged at a blocked UI is observable, but is not running work. */
export async function acknowledgedOutcome(
  name: string,
  expected: { kind: HostKind; pane: string; cwd: string },
  fallback: LifecycleOutcome,
): Promise<LifecycleOutcome> {
  const live = await inspectAgent(name);
  if (!live) throw new CommandError("agent disappeared before prompt acknowledgement was verified");
  requireIdentity(live, expected);
  if (live.status !== "blocked") return fallback;
  const result = await runCommand(
    [
      "herdr",
      "agent",
      "read",
      name,
      "--source",
      "recent-unwrapped",
      "--lines",
      "15",
      "--format",
      "text",
    ],
    { timeoutMs: 10_000, maxOutputBytes: 128 * 1024 },
  );
  if (result.exitCode !== 0 || result.timedOut || result.cancelled || result.truncated)
    throw new CommandError("cannot inspect blocked prompt acknowledgement", result.exitCode, true);
  return acknowledgedStatus(live.status, result.stdout, fallback);
}

export async function shutdownAgent(
  name: string,
  expected: { kind: HostKind; pane: string; cwd: string },
): Promise<LifecycleOutcome> {
  const live = await inspectAgent(name);
  if (live) requireIdentity(live, expected);
  const pane = await herdr(["pane", "process-info", "--pane", expected.pane], 10_000);
  const info = pane.process_info as
    | { shell_pid?: number; foreground_process_group_id?: number }
    | undefined;
  if (typeof info?.shell_pid !== "number")
    throw new CommandError("pane shell ownership unavailable");
  const owned = await ownedWorkload(expected.cwd, info.shell_pid);
  if (!live) return owned.length ? "cleanup-incomplete" : "exited";
  if (expected.kind === "opencode") {
    // Verified for --standalone: interrupt closes the owned runtime and its jobs.
    await herdr(["agent", "send-keys", name, "ctrl+c"], 10_000);
  } else {
    await herdr(["agent", "send-keys", name, "esc"], 10_000);
    // Codex's turn cancellation leaves tool jobs alive; /stop is a separate operation.
    if (expected.kind === "codex") {
      await herdr(
        ["agent", "wait", name, "--until", "idle", "--until", "done", "--timeout", "10000"],
        12_000,
      );
      await herdr(["agent", "prompt", name, "/stop"], 10_000);
      await herdr(
        ["agent", "wait", name, "--until", "idle", "--until", "done", "--timeout", "10000"],
        12_000,
      );
    }
    await herdr(["agent", "prompt", name, "/exit"], 10_000);
  }
  const until = Date.now() + 30_000;
  const wait = async (): Promise<LifecycleOutcome> => {
    if (!(await inspectAgent(name)))
      return (await verifyOwnedExit(owned)) &&
        (await ownedWorkload(expected.cwd, info.shell_pid)).length === 0
        ? "exited"
        : "cleanup-incomplete";
    if (Date.now() >= until) return "cleanup-incomplete";
    return Bun.sleep(1000).then(wait);
  };
  return wait();
}
