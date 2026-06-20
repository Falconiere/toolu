// Claude Code adapter for the activity lane. Maps a project root to its transcript
// store (`${CLAUDE_CONFIG_DIR:-~/.claude}/projects/<slug>`), picks the most-recent
// session, and reads the main transcript + every subagents/agent-*.jsonl to build
// the spawn tree. Slug derivation matches plugins/stats/scripts/lib/scan.sh:27.
// Never throws — a project with no resolvable store yields zero/empty results.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";

import type { DiscoveredProject } from "../discovery.ts";
import { extractResults, extractSpawns, readJsonl } from "../transcript.ts";
import type { ActivitySummary, AgentNode, LiveActivitySource } from "./source.ts";
import { type AgentMeta, buildTree, type ResultRec, type SpawnRec } from "./tree-builder.ts";

const EMPTY_SUMMARY: ActivitySummary = { total: 0, running: 0, errored: 0, stale: 0, deepest: 0 };

/** `${CLAUDE_CONFIG_DIR:-~/.claude}/projects` — the transcript store root. */
function projectsRoot(): string {
  const cfgDir = process.env.CLAUDE_CONFIG_DIR;
  const base = cfgDir && cfgDir.length > 0 ? cfgDir : join(homedir(), ".claude");
  return join(base, "projects");
}

/** Project root → Claude Code slug (every non-alphanumeric char → `-`). */
export function slugFor(root: string): string {
  return root.replace(/[^A-Za-z0-9]/g, "-");
}

interface SessionPaths {
  transcriptPath: string;
  subagentsDir: string;
  mtimeMs: number;
}

/** Sessions for a project, newest first; [] if the slug dir is absent. */
function sessionsFor(project: DiscoveredProject): SessionPaths[] {
  const dir = join(projectsRoot(), slugFor(project.root));
  let entries: string[] = [];
  try {
    entries = readdirSync(dir);
  } catch {
    return [];
  }
  const sessions: SessionPaths[] = [];
  for (const f of entries) {
    if (!f.endsWith(".jsonl")) continue;
    const transcriptPath = join(dir, f);
    let mtimeMs = 0;
    try {
      mtimeMs = statSync(transcriptPath).mtimeMs;
    } catch {
      continue;
    }
    sessions.push({
      transcriptPath,
      subagentsDir: join(dir, basename(f, ".jsonl"), "subagents"),
      mtimeMs,
    });
  }
  return sessions.sort((a, b) => b.mtimeMs - a.mtimeMs);
}

/** agentId from `agent-<id>.meta.json` / `agent-<id>.jsonl`. */
function agentIdOf(file: string): string {
  return basename(file).replace(/^agent-/, "").replace(/\.(meta\.json|jsonl)$/, "");
}

/** Read every meta sidecar in a subagents dir into AgentMeta records. */
function readMetas(subagentsDir: string): AgentMeta[] {
  let files: string[] = [];
  try {
    files = readdirSync(subagentsDir);
  } catch {
    return [];
  }
  const metas: AgentMeta[] = [];
  for (const f of files) {
    if (!f.endsWith(".meta.json")) continue;
    try {
      const m = JSON.parse(readFileSync(join(subagentsDir, f), "utf8")) as Record<string, unknown>;
      metas.push({
        agentId: agentIdOf(f),
        agentType: typeof m.agentType === "string" ? m.agentType : "",
        description: typeof m.description === "string" ? m.description : "",
        toolUseId: typeof m.toolUseId === "string" ? m.toolUseId : "",
      });
    } catch {
      // skip an unreadable/corrupt sidecar
    }
  }
  return metas.filter((m) => m.toolUseId.length > 0);
}

/** Gather spawn (tagged with owner) + result maps across main + each subagent transcript. */
function gather(
  session: SessionPaths,
  subagentFiles: string[],
): { spawns: Map<string, SpawnRec>; results: Map<string, ResultRec> } {
  const spawns = new Map<string, SpawnRec>();
  const results = new Map<string, ResultRec>();
  const ingest = (path: string, ownerAgentId: string | null): void => {
    const lines = readJsonl(path);
    for (const [id, s] of extractSpawns(lines)) spawns.set(id, { startedAt: s.startedAt, ownerAgentId });
    for (const [id, r] of extractResults(lines)) if (!results.has(id)) results.set(id, r);
  };
  ingest(session.transcriptPath, null);
  for (const f of subagentFiles) ingest(join(session.subagentsDir, f), agentIdOf(f));
  return { spawns, results };
}

/** Claude Code implementation of the activity-lane source. */
export const claudeCodeSource: LiveActivitySource = {
  countSpawned(project: DiscoveredProject): number {
    const newest = sessionsFor(project)[0];
    if (!newest) return 0;
    try {
      return readdirSync(newest.subagentsDir).filter((f) => f.endsWith(".meta.json")).length;
    } catch {
      return 0;
    }
  },

  tree(
    project: DiscoveredProject,
    nowMs: number,
    agentStuckSeconds: number,
  ): { agents: AgentNode[]; summary: ActivitySummary } {
    const newest = sessionsFor(project)[0];
    if (!newest || !existsSync(newest.subagentsDir)) return { agents: [], summary: { ...EMPTY_SUMMARY } };
    const metas = readMetas(newest.subagentsDir);
    if (metas.length === 0) return { agents: [], summary: { ...EMPTY_SUMMARY } };
    let subagentFiles: string[] = [];
    try {
      subagentFiles = readdirSync(newest.subagentsDir).filter((f) => f.endsWith(".jsonl"));
    } catch {
      subagentFiles = [];
    }
    const { spawns, results } = gather(newest, subagentFiles);
    return buildTree(metas, spawns, results, nowMs, agentStuckSeconds);
  },

  activityFingerprint(project: DiscoveredProject): number {
    const newest = sessionsFor(project)[0];
    if (!newest) return 0;
    let fp = newest.mtimeMs;
    try {
      // The dir mtime only moves when entries are added/removed, so also fold
      // each subagent transcript's mtime+size — otherwise an in-place append
      // (a child agent finishing) leaves the fingerprint unchanged and the live
      // tree shows that agent stuck "running".
      fp += statSync(newest.subagentsDir).mtimeMs;
      for (const f of readdirSync(newest.subagentsDir)) {
        if (!f.endsWith(".jsonl")) continue;
        const st = statSync(join(newest.subagentsDir, f));
        fp += st.mtimeMs + st.size;
      }
    } catch {
      // no subagents dir yet — transcript mtime alone is the signature
    }
    return Math.round(fp);
  },
};
