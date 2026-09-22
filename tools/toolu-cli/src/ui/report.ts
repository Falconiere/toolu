import type { CatalogState } from "../plugins/list";
import type { InstallStep } from "../plugins/install";
import type { RemoveStep } from "../plugins/remove";
import type { UpdateStep } from "../plugins/update";

const MARK: Record<string, string> = {
  installed: "+",
  already: "=",
  skew: "!",
  failed: "x",
  skipped: "-",
  updated: "+",
  current: "=",
};

function line(mark: string, name: string, detail: string): string {
  return `  ${mark} ${name.padEnd(16)} ${detail}\n`;
}

export function reportInstall(steps: readonly InstallStep[], dryRun: boolean): string {
  const header = dryRun ? "Planned commands (nothing was run):\n" : "";
  const body = steps
    .map((step) =>
      dryRun
        ? `  ${step.argv.join(" ")}\n`
        : line(MARK[step.outcome] ?? "?", step.name, step.detail),
    )
    .join("");
  return `${header}${body}`;
}

export function reportList(entries: readonly CatalogState[]): string {
  return entries
    .map((entry) =>
      line(
        entry.installed ? "=" : "-",
        entry.name,
        entry.installed
          ? `${entry.version ?? "?"}${entry.enabled ? "" : " (disabled)"}`
          : "not installed",
      ),
    )
    .join("");
}

export function reportRemove(steps: readonly RemoveStep[]): string {
  return steps.map((step) => line(step.removed ? "-" : "x", step.name, step.detail)).join("");
}

export function reportUpdate(steps: readonly UpdateStep[]): string {
  return steps.map((step) => line(MARK[step.outcome] ?? "?", step.name, step.detail)).join("");
}

/** Exit 1 when any step failed, so a caller's shell sees the failure. */
export function anyFailed(
  steps: readonly { readonly outcome?: string; readonly removed?: boolean }[],
): boolean {
  return steps.some((step) => step.outcome === "failed" || step.removed === false);
}
