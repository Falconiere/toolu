/**
 * The explicit status report for Codex and OpenCode skills. Only fields
 * available from local repository and toolu state: no account, model, effort
 * or context window.
 */
import type { ProjectStatus } from "./collect.ts";

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
  const lines = [`Host: ${hostLabel}`, ...repositoryLines(status), gateLine(status.gate)];
  if (status.comemory_count !== null) lines.push(`Comemory: ${status.comemory_count} memories`);
  if (status.jev.status === "ready") lines.push("Jev: ready");
  if (status.jev.status === "unavailable") lines.push(`Jev: unavailable — ${status.jev.reason}`);
  return `${lines.join("\n")}\n`;
}
