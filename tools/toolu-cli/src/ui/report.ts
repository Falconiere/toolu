import type { Host } from "../args/types";
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

export function reportInstall(steps: readonly InstallStep[], dryRun: boolean, host?: Host): string {
  if (dryRun && host === "opencode") return reportPlanned(steps);
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

/** Install report with a section per host when more than one was targeted. */
export function reportInstallByHost(
  sections: readonly { readonly host: Host; readonly steps: readonly InstallStep[] }[],
  dryRun: boolean,
): string {
  if (sections.length === 0) return "";
  if (sections.length === 1) {
    const only = sections[0];
    return only === undefined ? "" : reportInstall(only.steps, dryRun, only.host);
  }
  return sections
    .map((section) => `${section.host}:\n${reportInstall(section.steps, dryRun, section.host)}`)
    .join("\n");
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

/** Every step shape a verb can report. */
type AnyStep = InstallStep | RemoveStep | UpdateStep;

function failed(step: AnyStep): boolean {
  return "removed" in step ? !step.removed : step.outcome === "failed";
}

/** Exit 1 when any step failed, so a caller's shell sees the failure. */
export function anyFailed(steps: readonly AnyStep[]): boolean {
  return steps.some(failed);
}

/** A dry run of a file-editing host: the planned changes, then any refused step. */
export function reportPlanned(steps: readonly AnyStep[]): string {
  const planned = steps.flatMap((step) => (step.plan === undefined ? [] : [`  ${step.plan}\n`]));
  const refused = steps.filter(failed).map((step) => line("x", step.name, step.detail));
  const body = planned.length > 0 ? planned.join("") : "  (no changes)\n";
  return `Planned changes (nothing was written):\n${body}${refused.join("")}`;
}
