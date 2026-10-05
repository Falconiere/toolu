/**
 * The per-plugin support section of docs/opencode.md (#363). Each plugin's
 * status comes from the capability matrix; its CI checks are the actual-host
 * acceptance checks dedicated to it, which `bun run test:opencode` requires to
 * pass. Rendering assumes every check passes, so the section lists exactly what
 * CI demands rather than what one run happened to observe.
 */
import { AXES, isUsed, type Matrix, type ProbeResults } from "../opencode-host/schema.ts";
import { catalogNames, coverage } from "./checks.ts";
import { acceptanceChecks } from "./families.ts";

export const SUPPORT_START = "<!-- opencode-support:start -->";
export const SUPPORT_END = "<!-- opencode-support:end -->";

type Row = Matrix["plugins"][string];

/** Plugin → ids of the actual-host checks dedicated to it, from the acceptance registry. */
export function requiredChecks(committed: ProbeResults): Record<string, string[]> {
  const all = acceptanceChecks(committed).map((check) => ({ check, pass: true }));
  return coverage(all, catalogNames());
}

/** Each limitation of a row: a used axis short of supported, or a matrix note. */
function limitations(row: Row): string[] {
  const cells = AXES.flatMap((axis) => {
    const cell = row.axes[axis];
    if (!isUsed(cell) || cell.status === "supported") return [];
    const alternative = cell.alternative === undefined ? "" : ` Alternative: ${cell.alternative}.`;
    return [`${axis} (${cell.status}): ${cell.use}.${alternative}`];
  });
  return [...cells, ...(row.notes ?? []).map((note) => `${note.need}.`)];
}

function status(row: Row): string {
  const blocked = AXES.some((axis) => {
    const cell = row.axes[axis];
    return isUsed(cell) && cell.releaseBlocker === true;
  });
  if (blocked) return "Blocked";
  return limitations(row).length === 0 ? "Supported" : "Supported with limitations";
}

/** Ids grouped by their prefix: a lone id verbatim, a larger group `prefix.*` with a count. */
function checkGroups(ids: readonly string[]): string {
  if (ids.length === 0) return "none";
  const prefixes = [...new Set(ids.map((id) => id.split(".")[0] ?? id))];
  return prefixes
    .map((prefix) => {
      const group = ids.filter((id) => (id.split(".")[0] ?? id) === prefix);
      return group.length === 1 ? `\`${group[0] ?? prefix}\`` : `\`${prefix}.*\` (${group.length})`;
    })
    .join(", ");
}

export function renderSupport(matrix: Matrix, checks: Record<string, string[]>): string {
  const names = Object.keys(matrix.plugins).toSorted();
  const rows = names.map((name) => {
    const row = matrix.plugins[name];
    const label = row === undefined ? "Missing from the matrix" : status(row);
    return `| ${name} | ${label} | ${checkGroups(checks[name] ?? [])} |`;
  });
  const notes = names.flatMap((name) => {
    const row = matrix.plugins[name];
    return row === undefined ? [] : limitations(row).map((text) => `- **${name}** — ${text}`);
  });
  return [
    "| Plugin | Status on OpenCode | Dedicated CI checks |",
    "|---|---|---|",
    ...rows,
    "",
    "**Host-specific limitations**",
    "",
    ...(notes.length === 0 ? ["None."] : notes),
  ].join("\n");
}
