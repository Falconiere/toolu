// Assemble the multi-project payload. The sidebar (buildSummaries) is cheap:
// ledger reads + a readdir for the agent count, NEVER a transcript parse and
// never git — so N active projects stay fast. The deep work (git diff_sha via
// state.ts:buildState + the agent tree) happens only for the selected project.

import { readFileSync } from "node:fs";

import type { DashboardConfig } from "./config.ts";
import { discoverProjects, type DiscoveredProject } from "./discovery.ts";
import { buildState, type DashboardState, type Ledger } from "./state.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "./activity/source.ts";

/** Cheap per-project rollup for the sidebar. */
export interface ProjectSummary {
  id: string;
  label: string;
  root: string;
  branch: string;
  planCounts: { pending: number; running: number; red: number; green: number };
  planStale: number;
  agentsSpawned: number;
  dot: "running" | "blocked" | "done" | "idle";
  lastActiveMs: number;
}

/** Deep detail for the selected project: plan lane + activity tree + summary. */
export interface SelectedProjectDetail {
  id: string;
  plan: DashboardState;
  agents: AgentNode[];
  activity: ActivitySummary;
}

/** The full payload served at /api/state and pushed over SSE. */
export interface MultiDashboardState {
  projects: ProjectSummary[];
  selected: SelectedProjectDetail | null;
  serverTime: string;
}

/** Parse a ledger file, or null if absent/corrupt (never throws). */
function readLedger(path: string): Ledger | null {
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as Ledger;
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch {
    return null;
  }
}

/** Tally step statuses into the four kanban buckets. */
function countSteps(ledger: Ledger | null): ProjectSummary["planCounts"] {
  const counts = { pending: 0, running: 0, red: 0, green: 0 };
  for (const s of ledger?.steps ?? []) {
    if (s.status === "running") counts.running++;
    else if (s.status === "red") counts.red++;
    else if (s.status === "green") counts.green++;
    else counts.pending++;
  }
  return counts;
}

/** Status dot from the plan counts: running > blocked > done > idle. */
function dotFor(counts: ProjectSummary["planCounts"]): ProjectSummary["dot"] {
  if (counts.running > 0) return "running";
  if (counts.red > 0) return "blocked";
  const total = counts.pending + counts.running + counts.red + counts.green;
  if (total > 0 && counts.green === total) return "done";
  return "idle";
}

/** True if the project has a running step or a recently-modified ledger. */
function isActive(p: DiscoveredProject, counts: ProjectSummary["planCounts"], cfg: DashboardConfig, nowMs: number): boolean {
  if (counts.running > 0) return true;
  return nowMs - p.ledgerMtimeMs <= cfg.activeWithinHours * 3600 * 1000;
}

/** Sidebar rollups for every active project — ledger + readdir only, NO transcript parse, NO git. */
export function buildSummaries(
  cfg: DashboardConfig,
  src: LiveActivitySource,
  discovered: DiscoveredProject[] = discoverProjects(cfg),
): ProjectSummary[] {
  const nowMs = Date.now();
  const out: ProjectSummary[] = [];
  for (const p of discovered) {
    const ledger = readLedger(p.ledgerPath);
    const planCounts = countSteps(ledger);
    if (!isActive(p, planCounts, cfg, nowMs)) continue;
    out.push({
      id: p.id,
      label: p.label,
      root: p.root,
      branch: p.branch,
      planCounts,
      planStale: typeof ledger?.summary?.stale === "number" ? ledger.summary.stale : 0,
      agentsSpawned: src.countSpawned(p),
      dot: dotFor(planCounts),
      lastActiveMs: p.ledgerMtimeMs,
    });
  }
  return out.sort((a, b) => b.lastActiveMs - a.lastActiveMs);
}

/** Deep detail for one project id, or null if no such active/discovered project. */
export function buildSelectedDetail(
  cfg: DashboardConfig,
  src: LiveActivitySource,
  projectId: string,
  nowMs: number,
): SelectedProjectDetail | null {
  const project = discoverProjects(cfg).find((p) => p.id === projectId);
  if (!project) return null;
  const plan = buildState({
    ledgerPath: project.ledgerPath,
    repoRoot: project.root,
    stuckThresholdSeconds: cfg.stuckThresholdSeconds,
  });
  const { agents, summary } = src.tree(project, nowMs, cfg.agentStuckSeconds);
  return { id: projectId, plan, agents, activity: summary };
}

/** The default selection: an explicit choice, else the most-recently-active project. */
export function defaultSelectedId(
  projects: ProjectSummary[],
  selectedId: string | null,
): string | null {
  return selectedId ?? projects[0]?.id ?? null;
}

/** Full payload from an already-built summaries list, so the watcher can reuse one scan. */
export function buildMultiStateFrom(
  cfg: DashboardConfig,
  src: LiveActivitySource,
  projects: ProjectSummary[],
  selectedId: string | null,
  nowMs: number,
): MultiDashboardState {
  const wanted = defaultSelectedId(projects, selectedId);
  const selected = wanted ? buildSelectedDetail(cfg, src, wanted, nowMs) : null;
  return { projects, selected, serverTime: new Date(nowMs).toISOString() };
}

/** Full payload: summaries + selected detail (defaulting to the most-recently-active). */
export function buildMultiState(
  cfg: DashboardConfig,
  src: LiveActivitySource,
  selectedId: string | null,
  nowMs: number,
): MultiDashboardState {
  return buildMultiStateFrom(cfg, src, buildSummaries(cfg, src), selectedId, nowMs);
}
