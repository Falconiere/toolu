// Assemble the multi-project payload. The sidebar (buildSummaries) is cheap:
// ledger reads + a readdir for the agent count, NEVER a transcript parse and
// never git — so N active projects stay fast. The deep work (git diff_sha via
// state.ts:buildState + the agent tree) happens only for the selected project.

import { readFileSync } from "node:fs";

import type { DashboardConfig } from "./config.ts";
import { discoverProjects, type DiscoveredProject } from "./discovery.ts";
import { buildState, type DashboardState, type Ledger } from "./state.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "./activity/source.ts";
import { backfillRepo } from "./activity/backfill.ts";
import {
  applyRetention,
  isEnoent,
  listSessions,
  readSession,
  type SessionSummary,
} from "./activity/store.ts";
import { buildTree } from "./activity/tree-builder.ts";

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

/** Deep detail for the selected project: plan lane + live activity tree + summary
 *  + the persisted session history (the browsable past-runs lane). */
export interface SelectedProjectDetail {
  id: string;
  plan: DashboardState;
  agents: AgentNode[];
  activity: ActivitySummary;
  /** Past sessions for this project from the persistent store (newest first). */
  sessions: SessionSummary[];
}

/** One past session reassembled into the same tree shape the live lane renders. */
export interface SessionDetail {
  tree: AgentNode[];
  summary: ActivitySummary;
}

/** The full payload served at /api/state and pushed over SSE. */
export interface MultiDashboardState {
  projects: ProjectSummary[];
  selected: SelectedProjectDetail | null;
  serverTime: string;
}

/** A parsed ledger is accepted when it is a non-null object (the readers below
 *  use optional access); a type guard, not a cast assertion. */
function isLedgerLike(v: unknown): v is Ledger {
  return typeof v === "object" && v !== null;
}

/** Parse a ledger file, or null if absent/corrupt (never throws). */
function readLedger(path: string): Ledger | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
  } catch (err) {
    // A missing ledger is the normal "project has no plan" case; surface
    // anything else (corrupt JSON, I/O) before treating it as absent. Reuses the
    // store's isEnoent so both readers classify fs errors the same way.
    if (!isEnoent(err)) console.error(`dashboard: reading ledger ${path} failed — treating as absent (${String(err)})`);
    return null;
  }
  return isLedgerLike(parsed) ? parsed : null;
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

/** Persist this project's sessions into the store, prune by retention policy, and
 *  return its session summaries (newest first). backfillRepo is mtime-cached so a
 *  repeat call on an unchanged repo is a no-op. Never throws: any store/backfill
 *  failure degrades to whatever the index already holds (possibly []). */
function syncSessions(cfg: DashboardConfig, project: DiscoveredProject, nowMs: number): SessionSummary[] {
  try {
    backfillRepo(project);
    applyRetention(
      project.id,
      {
        retentionDays: cfg.retentionDays,
        maxSessionsPerProject: cfg.maxSessionsPerProject,
      },
      nowMs,
    );
  } catch (err) {
    // Store work is best-effort: fall through to whatever listSessions can read
    // rather than failing the whole dashboard tick. Report it so a persistently
    // broken backfill/retention is visible instead of silently serving stale data.
    console.error(`dashboard: session sync for ${project.id} failed — serving the stored index (${String(err)})`);
  }
  return listSessions(project.id);
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
  const sessions = syncSessions(cfg, project, nowMs);
  return { id: projectId, plan, agents, activity: summary, sessions };
}

/** Reassemble one persisted past session into a tree + summary, in the exact
 *  shape the live lane renders. Reads tolerantly (a missing/empty session log
 *  yields an empty tree, never throws), then runs #117's buildTree. */
export function buildSessionDetail(
  projectId: string,
  sessionId: string,
  nowMs: number,
  agentStuckSeconds: number,
): SessionDetail {
  const { metas, spawns, results } = readSession(projectId, sessionId);
  const { agents, summary } = buildTree(metas, spawns, results, nowMs, agentStuckSeconds);
  return { tree: agents, summary };
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
