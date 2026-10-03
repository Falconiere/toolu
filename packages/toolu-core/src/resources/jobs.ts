/** Expensive work runs under a machine-wide lease, including background jobs. */
import { randomUUID } from "node:crypto";
import {
  processGroupAlive,
  runCommand,
  signalProcessGroup,
  type RunCommandOptions,
  type RunCommandResult,
} from "../process/process.ts";
import { acquireLease, patchLease, readResourceState, releaseLease } from "./resources.ts";
import { type ResourceBinding } from "./binding.ts";

const FORWARDED_SIGNALS: readonly NodeJS.Signals[] = ["SIGINT", "SIGTERM", "SIGHUP"];

function noop(): void {}

/** Prefix the job's stdin with the go line, keeping binary input byte-exact. */
function withGoSignal(stdin: string | Uint8Array | undefined): string | Uint8Array {
  if (stdin === undefined || typeof stdin === "string") return `toolu-go\n${stdin ?? ""}`;
  const prefix = new TextEncoder().encode("toolu-go\n");
  const bytes = new Uint8Array(prefix.length + stdin.length);
  bytes.set(prefix);
  bytes.set(stdin, prefix.length);
  return bytes;
}

function guardOwnedGroup(processGroupId: number): () => void {
  const onExit = (): void => signalProcessGroup(processGroupId, "SIGKILL");
  const listeners = FORWARDED_SIGNALS.map((signal): readonly [NodeJS.Signals, () => void] => [
    signal,
    () => {
      signalProcessGroup(processGroupId, "SIGKILL");
      release();
      process.kill(process.pid, signal);
    },
  ]);
  const release = (): void => {
    process.off("exit", onExit);
    for (const [signal, listener] of listeners) process.off(signal, listener);
  };
  process.on("exit", onExit);
  for (const [signal, listener] of listeners) process.on(signal, listener);
  return release;
}

export async function runManagedJob(
  argv: string[],
  binding: ResourceBinding,
  opts: RunCommandOptions = {},
): Promise<RunCommandResult> {
  const lease = await acquireLease(binding.root, {
    type: "job",
    key: `${binding.key}:${randomUUID()}`,
    stateDir: binding.stateDir,
    worktree: binding.worktree,
  });
  let group: number | undefined;
  let heartbeat: ReturnType<typeof setInterval> | undefined;
  let heartbeatFailure: unknown;
  let releaseGuard = noop;
  try {
    const result = await runCommand(
      [
        "bash",
        "-c",
        'IFS= read -r ack && [ "$ack" = toolu-go ] && exec "$@"',
        "toolu-job",
        ...argv,
      ],
      {
        ...opts,
        stdin: withGoSignal(opts.stdin),
        onSpawn: async (pid) => {
          group = pid;
          releaseGuard = guardOwnedGroup(pid);
          await patchLease(binding.root, lease.token, { groupPid: pid, stage: "running" });
          await opts.onSpawn?.(pid);
          const beat = async (): Promise<void> => {
            try {
              await patchLease(binding.root, lease.token, {
                heartbeatAt: new Date().toISOString(),
              });
              heartbeatFailure = undefined;
            } catch (error) {
              heartbeatFailure = error;
            }
          };
          heartbeat = setInterval(() => void beat(), 30_000);
        },
      },
    );
    if (heartbeatFailure !== undefined) throw heartbeatFailure;
    return result;
  } finally {
    releaseGuard();
    clearInterval(heartbeat);
    // Keep uncertain ownership visible; cancellation must prove no survivors.
    if (group === undefined || !processGroupAlive(group))
      await releaseLease(binding.root, lease.token);
    else await patchLease(binding.root, lease.token, { stage: "cleanup-incomplete" });
  }
}

export function activeJobs(root: string, worktree: string): boolean {
  return readResourceState(root).leases.some(
    (lease) => lease.type === "job" && lease.worktree === worktree,
  );
}
