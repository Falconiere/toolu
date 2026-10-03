const PROCESS_POLL_MS = 25;
const TERMINATE_GRACE_MS = 250;
const KILL_GRACE_MS = 1_000;
const PROCESS_TABLE_TIMEOUT_MS = 2_000;
const PROCESS_TABLE_MAX_BYTES = 8 * 1024 * 1024;

function errno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

function validateProcessGroupId(processGroupId: number): void {
  if (!Number.isSafeInteger(processGroupId) || processGroupId <= 0) {
    throw new Error("processGroupId must be a positive safe integer");
  }
}

function groupTarget(processGroupId: number): number {
  return process.platform === "win32" ? processGroupId : -processGroupId;
}

/** Send a signal to a detached command's process group. A group that already exited is success. */
export function signalProcessGroup(processGroupId: number, signal: NodeJS.Signals | number): void {
  validateProcessGroupId(processGroupId);
  try {
    process.kill(groupTarget(processGroupId), signal);
  } catch (error) {
    if (!errno(error, "ESRCH")) throw error;
  }
}

function signalProbe(processGroupId: number): boolean {
  try {
    process.kill(groupTarget(processGroupId), 0);
    return true;
  } catch (error) {
    if (errno(error, "ESRCH")) return false;
    if (errno(error, "EPERM")) return true;
    throw error;
  }
}

/** True when a detached command group contains a non-zombie process. */
export function processGroupAlive(processGroupId: number): boolean {
  validateProcessGroupId(processGroupId);
  if (process.platform === "win32") return signalProbe(processGroupId);
  const table = Bun.spawnSync(["ps", "-axo", "pgid=,stat="], {
    stdout: "pipe",
    stderr: "ignore",
    timeout: PROCESS_TABLE_TIMEOUT_MS,
    killSignal: "SIGKILL",
    maxBuffer: PROCESS_TABLE_MAX_BYTES,
  });
  if (table.exitCode !== 0) return signalProbe(processGroupId);
  const rows = new TextDecoder().decode(table.stdout).split("\n");
  for (const row of rows) {
    const match = /^\s*(\d+)\s+(\S+)/.exec(row);
    if (match === null || Number(match[1]) !== processGroupId) continue;
    if (!match[2]?.startsWith("Z")) return true;
  }
  return false;
}

async function waitForDeadGroup(processGroupId: number, deadline: number): Promise<boolean> {
  if (!processGroupAlive(processGroupId)) return true;
  const remaining = deadline - performance.now();
  if (remaining <= 0) return false;
  await Bun.sleep(Math.min(PROCESS_POLL_MS, remaining));
  return waitForDeadGroup(processGroupId, deadline);
}

/** Stop a process group gently, then forcibly, and verify that no live member remains. */
export async function terminateProcessGroup(processGroupId: number): Promise<void> {
  signalProcessGroup(processGroupId, "SIGTERM");
  if (await waitForDeadGroup(processGroupId, performance.now() + TERMINATE_GRACE_MS)) return;
  signalProcessGroup(processGroupId, "SIGKILL");
  if (!(await waitForDeadGroup(processGroupId, performance.now() + KILL_GRACE_MS))) {
    throw new Error(`process group ${processGroupId} survived SIGKILL`);
  }
}
