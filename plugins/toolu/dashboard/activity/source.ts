// The activity lane's runtime-agnostic contract. A LiveActivitySource turns a
// discovered project into (cheaply) a spawned-agent count for the sidebar, or
// (deeply) the full agent tree + summary for the selected project. Concrete
// adapters (e.g. claude-code.ts) live alongside; per-runtime specifics stay there.

import type { DiscoveredProject } from "../discovery.ts";

/** Lifecycle status of one agent node. `stale` = unpaired past the stuck threshold. */
export type AgentStatus = "running" | "done" | "error" | "stale";

/** One node in the spawn tree: an agent and the agents it spawned. */
export interface AgentNode {
  agentId: string;
  /** The tool_use id that spawned this agent (join key to its parent). */
  toolUseId: string;
  /** The spawning tool_use id of this node's parent agent; null if top-level. */
  parentToolUseId: string | null;
  label: string; // meta.json.description
  agentType: string; // meta.json.agentType
  status: AgentStatus;
  startedAt: string | null;
  endedAt: string | null;
  durationMs: number | null;
  children: AgentNode[];
}

/** Aggregate counts over a project's agent tree, for the selected-project header. */
export interface ActivitySummary {
  total: number;
  running: number;
  errored: number;
  stale: number;
  deepest: number; // max tree depth
}

/** Read-only source of live agent activity for a project. */
export interface LiveActivitySource {
  /** Parse-free: count of spawned agents via a readdir of the subagents dir.
   *  MUST NOT read any transcript. Feeds the sidebar. 0 when no data resolves. */
  countSpawned(project: DiscoveredProject): number;
  /** Deep: parse the transcripts, pair tool_use↔tool_result, and build the tree
   *  plus its summary. Called only for the selected project. */
  tree(
    project: DiscoveredProject,
    nowMs: number,
    agentStuckSeconds: number,
  ): { agents: AgentNode[]; summary: ActivitySummary };
  /** Optional cheap stat-based signature (no parse) of the project's live activity,
   *  so the watcher can detect new/updated agents without rebuilding. 0 when absent. */
  activityFingerprint?(project: DiscoveredProject): number;
}
