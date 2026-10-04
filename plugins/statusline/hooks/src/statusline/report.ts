/**
 * The explicit status report for Codex and OpenCode skills. Only fields
 * available from local repository and toolu state: no account, model, effort
 * or context window. OpenCode adds toolu's readiness from the adapter's status
 * record (#359), with a next step whenever toolu is not ready or not recorded.
 */
import type { OpencodeStatusRecord } from "@toolu/core/startup";
import type { ProjectStatus, TooluStatus } from "./collect.ts";

const HOST_LABEL: Record<ProjectStatus["host"], string> = {
  claude: "Claude Code",
  codex: "Codex",
  opencode: "OpenCode",
};

function repositoryLines(status: ProjectStatus): string[] {
  if (status.repo_root === "") return [`Folder: ${status.folder} (not a git repository)`];
  const branch =
    (status.branch === "" ? "Branch: detached HEAD" : `Branch: ${status.branch}`) +
    (status.ahead > 0 ? ` (ahead ${status.ahead})` : "") +
    (status.behind > 0 ? ` (behind ${status.behind})` : "");
  const { staged, unstaged, untracked } = status.working_tree;
  const tree =
    staged === 0 && unstaged === 0 && untracked === 0
      ? "Working tree: clean"
      : `Working tree: staged ${staged}, unstaged ${unstaged}, untracked ${untracked}`;
  return [`Repository: ${status.repo_root}`, branch, tree];
}

const SELECTION_LABEL: Record<NonNullable<OpencodeStatusRecord["selection"]>, string> = {
  project: "project selection",
  global: "global selection",
  default: "all installed plugins",
};

function tooluLines(toolu: TooluStatus): string[] {
  if (toolu.status === "") return [];
  if (toolu.status !== "recorded") {
    return toolu.status === "missing"
      ? [
          `toolu: no startup record at ${toolu.path} — start OpenCode with the toolu plugin in this project, then run the status skill again`,
        ]
      : [`toolu: unreadable startup record at ${toolu.path} — restart OpenCode to rewrite it`];
  }
  const { record } = toolu;
  const recorded = `Startup record: ${toolu.path}, written ${record.written} for ${record.project}`;
  if (record.status === "not-ready") {
    return [
      `toolu: not ready — ${record.reason ?? "no reason recorded"}; every tool call stays denied until OpenCode restarts with the cause fixed`,
      recorded,
    ];
  }
  const artifacts = record.plugins.reduce((sum, plugin) => sum + plugin.artifacts, 0);
  const source = record.selection === undefined ? "" : ` (${SELECTION_LABEL[record.selection]})`;
  const plugins = record.plugins.map((plugin) =>
    plugin.entries.length === 0 ? plugin.name : `${plugin.name} (${plugin.entries.join(", ")})`,
  );
  const lines = [
    `toolu: ready — ${record.plugins.length} plugins${source}, ${artifacts} startup artifacts`,
    `Plugins: ${plugins.length === 0 ? "none" : plugins.join(", ")}`,
  ];
  if (record.notes.length > 0) lines.push(`Startup notes: ${record.notes.join("; ")}`);
  return [...lines, recorded];
}

function gateLine(gate: ProjectStatus["gate"]): string {
  if (gate.status === "failing") {
    return gate.reason === "" ? "Quality gate: failing" : `Quality gate: failing — ${gate.reason}`;
  }
  return gate.status === "passing" ? "Quality gate: passing" : "Quality gate: no recorded state";
}

/** The report, one field per line, newline-terminated. */
export function reportText(status: ProjectStatus): string {
  const hostLabel = HOST_LABEL[status.host];
  if (hostLabel === undefined)
    throw new Error(`unsupported statusline report host: ${status.host}`);
  const lines = [
    `Host: ${hostLabel}`,
    ...tooluLines(status.toolu),
    ...repositoryLines(status),
    gateLine(status.gate),
  ];
  if (status.comemory_count !== null) lines.push(`Comemory: ${status.comemory_count} memories`);
  if (status.jev.status === "ready") lines.push("Jev: ready");
  if (status.jev.status === "unavailable") lines.push(`Jev: unavailable — ${status.jev.reason}`);
  return `${lines.join("\n")}\n`;
}
