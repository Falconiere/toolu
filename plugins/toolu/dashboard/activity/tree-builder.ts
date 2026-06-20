// Pure assembly of the agent spawn tree from already-extracted meta + spawn +
// result records. No filesystem access — claude-code.ts gathers the inputs and
// hands them here, so this stays trivially testable and runtime-agnostic.

import type { ActivitySummary, AgentNode, AgentStatus } from "./source.ts";

/** One agent's identity, from its subagents/agent-<id>.meta.json sidecar. */
export interface AgentMeta {
  agentId: string;
  agentType: string;
  description: string;
  toolUseId: string;
}

/** Where a spawning tool_use was found: `ownerAgentId` null ⟹ the main transcript. */
export interface SpawnRec {
  startedAt: string | null;
  ownerAgentId: string | null;
}

/** A tool_result for a spawn. */
export interface ResultRec {
  endedAt: string | null;
  isError: boolean;
}

/** Difference of two ISO timestamps in ms, or null if either is missing/invalid. */
export function durationMs(start: string | null, end: string | null): number | null {
  if (!start || !end) return null;
  const a = Date.parse(start);
  const b = Date.parse(end);
  if (Number.isNaN(a) || Number.isNaN(b)) return null;
  return b - a;
}

/** Status from result presence, falling back to running/stale by age when unpaired. */
function statusOf(
  result: ResultRec | undefined,
  startedAt: string | null,
  nowMs: number,
  agentStuckSeconds: number,
): AgentStatus {
  if (result) return result.isError ? "error" : "done";
  if (startedAt) {
    const startMs = Date.parse(startedAt);
    if (!Number.isNaN(startMs) && (nowMs - startMs) / 1000 > agentStuckSeconds) return "stale";
  }
  return "running";
}

/** Max depth of a forest (1 for a flat list of leaves), with a cycle guard keyed
 *  on the unique agentId (toolUseId is not guaranteed unique across transcripts). */
function maxDepth(nodes: AgentNode[], seen: Set<string>): number {
  let deepest = 0;
  for (const n of nodes) {
    if (seen.has(n.agentId)) continue;
    seen.add(n.agentId);
    deepest = Math.max(deepest, 1 + maxDepth(n.children, seen));
  }
  return deepest;
}

/** Build the agent tree + summary. Top-level = spawned from main (or a missing
 *  owner); nested = spawned inside another known agent's transcript. */
export function buildTree(
  metas: AgentMeta[],
  spawns: Map<string, SpawnRec>,
  results: Map<string, ResultRec>,
  nowMs: number,
  agentStuckSeconds: number,
): { agents: AgentNode[]; summary: ActivitySummary } {
  const metaByAgentId = new Map(metas.map((m) => [m.agentId, m]));
  const nodeByTool = new Map<string, AgentNode>();
  const nodes: AgentNode[] = [];

  for (const meta of metas) {
    const spawn = spawns.get(meta.toolUseId);
    const result = results.get(meta.toolUseId);
    const startedAt = spawn?.startedAt ?? null;
    const endedAt = result?.endedAt ?? null;
    const owner = spawn?.ownerAgentId ?? null;
    const parentMeta = owner ? metaByAgentId.get(owner) : undefined;
    const node: AgentNode = {
      agentId: meta.agentId,
      toolUseId: meta.toolUseId,
      parentToolUseId: parentMeta ? parentMeta.toolUseId : null,
      label: meta.description,
      agentType: meta.agentType,
      status: statusOf(result, startedAt, nowMs, agentStuckSeconds),
      startedAt,
      endedAt,
      durationMs: durationMs(startedAt, endedAt),
      children: [],
    };
    nodes.push(node);
    nodeByTool.set(node.toolUseId, node);
  }

  const roots: AgentNode[] = [];
  for (const node of nodes) {
    const parent = node.parentToolUseId ? nodeByTool.get(node.parentToolUseId) : undefined;
    if (parent && parent !== node) parent.children.push(node);
    else roots.push(node);
  }

  const summary: ActivitySummary = {
    total: nodes.length,
    running: nodes.filter((n) => n.status === "running").length,
    errored: nodes.filter((n) => n.status === "error").length,
    stale: nodes.filter((n) => n.status === "stale").length,
    deepest: maxDepth(roots, new Set()),
  };
  return { agents: roots, summary };
}
