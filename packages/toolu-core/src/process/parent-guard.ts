import { signalProcessGroup } from "./process-group.ts";

const PARENT_SIGNALS: readonly NodeJS.Signals[] = ["SIGINT", "SIGTERM", "SIGHUP"];
const parentOwnedGroups = new Set<number>();
let parentGuardInstalled = false;

function killParentOwnedGroups(): void {
  for (const processGroupId of parentOwnedGroups) signalProcessGroup(processGroupId, "SIGKILL");
}

function removeParentGuard(): void {
  if (!parentGuardInstalled) return;
  process.off("exit", killParentOwnedGroups);
  for (const signal of PARENT_SIGNALS) process.off(signal, forwardParentSignal);
  parentGuardInstalled = false;
}

function forwardParentSignal(signal: NodeJS.Signals): void {
  killParentOwnedGroups();
  removeParentGuard();
  process.kill(process.pid, signal);
}

/** Keep a detached command from outliving a parent that exits or receives a termination signal. */
export function guardParentLifecycle(processGroupId: number): () => void {
  parentOwnedGroups.add(processGroupId);
  if (!parentGuardInstalled) {
    process.on("exit", killParentOwnedGroups);
    for (const signal of PARENT_SIGNALS) process.on(signal, forwardParentSignal);
    parentGuardInstalled = true;
  }
  return (): void => {
    parentOwnedGroups.delete(processGroupId);
    if (parentOwnedGroups.size === 0) removeParentGuard();
  };
}
