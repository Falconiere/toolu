/** Portable inventory of workload processes owned by one worktree. */
import { readFileSync, readlinkSync, readdirSync, realpathSync, statSync } from "node:fs";

const INVENTORY_TIMEOUT_MS = 5_000;
const INVENTORY_MAX_BYTES = 8 * 1024 * 1024;

export type OwnedWorkload = { pid: number; group: number; started: string };

type ProcessRecord = OwnedWorkload & {
  parent: number;
  cwd: string | undefined;
  cwdUnavailable?: boolean;
};

function errno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

function linuxRecord(pid: number): ProcessRecord | null {
  let stat: string;
  try {
    stat = readFileSync(`/proc/${pid}/stat`, "utf8");
  } catch (error) {
    if (errno(error, "ENOENT")) return null;
    throw new Error(`cannot inspect process ${pid}`, { cause: error });
  }
  const close = stat.lastIndexOf(")");
  const fields =
    close < 0
      ? []
      : stat
          .slice(close + 2)
          .trim()
          .split(/\s+/);
  const state = fields[0];
  const parent = Number(fields[1]);
  const group = Number(fields[2]);
  const started = fields[19];
  if (!state || !Number.isSafeInteger(parent) || !Number.isSafeInteger(group) || !started) {
    throw new Error(`invalid /proc stat for process ${pid}`);
  }
  if (state === "Z") return null;
  return { pid, parent, group, started: `linux:${started}`, cwd: undefined };
}

function linuxProcesses(): ProcessRecord[] {
  if (process.platform !== "linux") throw new Error("Linux process inventory is unavailable");
  const uid = process.getuid?.();
  if (uid === undefined) throw new Error("Linux process uid is unavailable");
  const records: ProcessRecord[] = [];
  for (const entry of readdirSync("/proc", { withFileTypes: true })) {
    if (!entry.isDirectory() || !/^\d+$/.test(entry.name)) continue;
    let owner: number;
    try {
      owner = statSync(`/proc/${entry.name}`).uid;
    } catch (error) {
      if (errno(error, "ENOENT")) continue;
      throw new Error(`cannot inspect process owner ${entry.name}`, { cause: error });
    }
    const record = linuxRecord(Number(entry.name));
    if (record === null) continue;
    // Foreign-UID descendants still need their parent and birth identity.
    if (owner === uid) {
      try {
        record.cwd = readlinkSync(`/proc/${record.pid}/cwd`);
      } catch (error) {
        if (errno(error, "EACCES") || errno(error, "EPERM")) record.cwdUnavailable = true;
        else if (!errno(error, "ENOENT")) {
          throw new Error(`cannot inspect process ${record.pid} cwd`, { cause: error });
        }
      }
    } else {
      record.cwdUnavailable = true;
    }
    records.push(record);
  }
  return records;
}

function command(argv: string[]): string {
  let result: ReturnType<typeof Bun.spawnSync>;
  try {
    result = Bun.spawnSync(argv, {
      stdout: "pipe",
      stderr: "pipe",
      timeout: INVENTORY_TIMEOUT_MS,
      killSignal: "SIGKILL",
      maxBuffer: INVENTORY_MAX_BYTES,
    });
  } catch (error) {
    throw new Error(`process inventory command unavailable: ${argv[0]}`, { cause: error });
  }
  if (result.exitCode !== 0) {
    throw new Error(
      `process inventory command failed (${result.exitCode}): ${new TextDecoder().decode(result.stderr)}`,
    );
  }
  return new TextDecoder().decode(result.stdout);
}

function macIdentities(): ProcessRecord[] {
  const output = command(["ps", "-axo", "pid=,ppid=,pgid=,stat=,lstart="]);
  return output
    .split("\n")
    .map((line): ProcessRecord | null => {
      const match = /^\s*(\d+)\s+(\d+)\s+(\d+)\s+(\S+)\s+(.+?)\s*$/.exec(line);
      if (match === null || match[4]?.startsWith("Z")) return null;
      return {
        pid: Number(match[1]),
        parent: Number(match[2]),
        group: Number(match[3]),
        started: `mac:${match[5]}`,
        cwd: undefined,
      };
    })
    .filter((record): record is ProcessRecord => record !== null);
}

function macProcesses(): ProcessRecord[] {
  if (process.platform !== "darwin") throw new Error("macOS process inventory is unavailable");
  const cwdByPid = new Map<number, string>();
  let pid: number | undefined;
  for (const line of command(["lsof", "-n", "-P", "-d", "cwd", "-F", "pn"]).split("\n")) {
    if (line.startsWith("p")) pid = Number(line.slice(1));
    else if (line.startsWith("n") && pid !== undefined) cwdByPid.set(pid, line.slice(1));
  }
  return macIdentities().map((record) => ({ ...record, cwd: cwdByPid.get(record.pid) }));
}

function processes(): ProcessRecord[] {
  if (process.platform === "linux") return linuxProcesses();
  if (process.platform === "darwin") return macProcesses();
  throw new Error(`process inventory is unavailable on ${process.platform}`);
}

function inside(cwd: string | undefined, worktree: string): boolean {
  return cwd === worktree || cwd?.startsWith(`${worktree}/`) === true;
}

export async function ownedWorkload(worktree: string, shellPid?: number): Promise<OwnedWorkload[]> {
  const root = realpathSync(worktree);
  const inventory = processes();
  const excluded = new Set(
    [process.pid, shellPid].filter((pid): pid is number => pid !== undefined),
  );
  const owned = new Set(
    inventory
      .filter((record) => !excluded.has(record.pid) && inside(record.cwd, root))
      .map((record) => record.pid),
  );
  const addDescendants = (): boolean => {
    let changed = false;
    for (const record of inventory) {
      if (!excluded.has(record.pid) && owned.has(record.parent) && !owned.has(record.pid)) {
        owned.add(record.pid);
        changed = true;
      }
    }
    return changed;
  };
  while (addDescendants()) {}
  const ownedGroups = new Set(
    inventory.filter((record) => owned.has(record.pid)).map((record) => record.group),
  );
  for (const record of inventory) {
    if (!owned.has(record.pid) && record.cwdUnavailable && ownedGroups.has(record.group)) {
      throw new Error(`cannot determine worktree ownership of process ${record.pid}`);
    }
  }
  return inventory
    .filter((record) => owned.has(record.pid))
    .map(({ pid, group, started }) => ({ pid, group, started }))
    .sort((left, right) => left.pid - right.pid);
}

/** True only when every recorded PID is gone or belongs to a later process incarnation. */
export async function verifyOwnedExit(records: readonly OwnedWorkload[]): Promise<boolean> {
  if (records.length === 0) return true;
  if (process.platform === "linux") {
    return records.every((record) => linuxRecord(record.pid)?.started !== record.started);
  }
  if (process.platform !== "darwin")
    throw new Error(`process inventory is unavailable on ${process.platform}`);
  const current = new Map(macIdentities().map((record) => [record.pid, record.started]));
  return records.every((record) => current.get(record.pid) !== record.started);
}
