// One-shot, current-repo-only transcript backfill into the persistent store.
// backfillRepo finds a project's sessions under `${CLAUDE_CONFIG_DIR:-~/.claude}/
// projects/<slug>/` (slug via #117's slugFor), parses each main transcript + its
// subagents exactly as the live claude-code.ts adapter does (readMetas + gather +
// transcript.ts extractSpawns/extractResults/readJsonl, feeding #117's buildTree
// shapes), and PERSISTS per session into the store as meta/spawn/result records,
// then rebuilds index.json with each session's summary. An mtime cache makes a 2nd
// run a no-op (idempotent by sessionId; the log is rewritten, not appended, so no
// dupes). Respects $CLAUDE_CONFIG_DIR / $HOME for the projects root and
// $TOOLU_ACTIVITY_DIR for the store base, so tests can redirect both. Never throws.

import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";

import type { DiscoveredProject } from "../discovery.ts";
import { extractResults, extractSpawns, readJsonl } from "../transcript.ts";
import { slugFor } from "./claude-code.ts";
import {
  type ActivityRecord,
  projectDir,
  readSession,
  type SessionData,
  summarizeSession,
  writeIndex,
} from "./store.ts";
import { type AgentMeta, type ResultRec, type SpawnRec } from "./tree-builder.ts";

/** Options for backfillRepo. `projectsRoot` overrides the transcript store root
 *  (`${CLAUDE_CONFIG_DIR:-~/.claude}/projects`); tests point it at a temp dir. */
export interface BackfillOptions {
  /** Override the projects root that holds `<slug>/<sessionId>.jsonl`. */
  projectsRoot?: string;
}

/** A discovered real session: its id + the mtime of its main transcript. */
interface DiscoveredSession {
  sessionId: string;
  mtimeMs: number;
  transcriptPath: string;
  subagentsDir: string;
}

/** `${CLAUDE_CONFIG_DIR:-~/.claude}/projects` — the transcript store root. */
function projectsRootDir(): string {
  const cfgDir = process.env.CLAUDE_CONFIG_DIR;
  const base = cfgDir && cfgDir.length > 0 ? cfgDir : join(process.env.HOME ?? homedir(), ".claude");
  return join(base, "projects");
}

/** agentId from `agent-<id>.meta.json` / `agent-<id>.jsonl` (matches claude-code.ts). */
function agentIdOf(file: string): string {
  return basename(file).replace(/^agent-/, "").replace(/\.(meta\.json|jsonl)$/, "");
}

/** Find a project's sessions under `<slug>/`, newest transcript first. [] if absent. */
function discoverSessions(slug: string): DiscoveredSession[] {
  let entries: string[];
  try {
    entries = readdirSync(slug);
  } catch {
    return []; // no transcripts for this project yet.
  }
  const out: DiscoveredSession[] = [];
  for (const f of entries) {
    if (!f.endsWith(".jsonl")) continue;
    const transcriptPath = join(slug, f);
    try {
      out.push({
        sessionId: basename(f, ".jsonl"),
        mtimeMs: statSync(transcriptPath).mtimeMs,
        transcriptPath,
        subagentsDir: join(slug, basename(f, ".jsonl"), "subagents"),
      });
    } catch {
      // unreadable: skip this session, never fatal.
    }
  }
  return out.sort((a, b) => b.mtimeMs - a.mtimeMs);
}

/** Read every meta sidecar in a subagents dir into AgentMeta records
 *  (agentId from filename; same shape #117's claude-code.ts readMetas builds). */
function readMetas(subagentsDir: string): AgentMeta[] {
  let files: string[];
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

/** Gather spawn (tagged with owner) + result maps across the main + each subagent
 *  transcript — identical to #117's claude-code.ts gather, feeding buildTree shapes. */
function gather(session: DiscoveredSession): {
  spawns: Map<string, SpawnRec>;
  results: Map<string, ResultRec>;
} {
  const spawns = new Map<string, SpawnRec>();
  const results = new Map<string, ResultRec>();
  const ingest = (path: string, ownerAgentId: string | null): void => {
    const lines = readJsonl(path);
    for (const [id, s] of extractSpawns(lines)) spawns.set(id, { startedAt: s.startedAt, ownerAgentId });
    for (const [id, r] of extractResults(lines)) if (!results.has(id)) results.set(id, r);
  };
  ingest(session.transcriptPath, null);
  let subagentFiles: string[] = [];
  try {
    subagentFiles = readdirSync(session.subagentsDir).filter((f) => f.endsWith(".jsonl"));
  } catch {
    subagentFiles = [];
  }
  for (const f of subagentFiles) ingest(join(session.subagentsDir, f), agentIdOf(f));
  return { spawns, results };
}

/** Flatten a parsed session into the store's meta/spawn/result record stream. */
function recordsFor(data: SessionData): ActivityRecord[] {
  const recs: ActivityRecord[] = [];
  for (const m of data.metas) recs.push({ kind: "meta", ...m });
  for (const [toolUseId, s] of data.spawns) {
    recs.push({ kind: "spawn", toolUseId, startedAt: s.startedAt, ownerAgentId: s.ownerAgentId });
  }
  for (const [toolUseId, r] of data.results) {
    recs.push({ kind: "result", toolUseId, endedAt: r.endedAt, isError: r.isError });
  }
  return recs;
}

/** Atomically (re)write a session log via temp + rename — used to make the 2nd
 *  run dup-free: a re-backfilled session's log is replaced, not appended. */
function rewriteSessionLog(dir: string, sessionId: string, recs: ActivityRecord[]): void {
  mkdirSync(dir, { recursive: true });
  const tmp = join(dir, `.${sessionId}.jsonl.${process.pid}.tmp`);
  writeFileSync(tmp, recs.map((r) => JSON.stringify(r)).join("\n") + "\n", "utf8");
  renameSync(tmp, join(dir, `${sessionId}.jsonl`));
}

/** Read the {sessionId: mtimeMs} backfill cache, or {} when absent/corrupt. */
function readCache(dir: string): Record<string, number> {
  try {
    const parsed = JSON.parse(readFileSync(join(dir, ".backfill-cache.json"), "utf8"));
    return parsed && typeof parsed === "object" ? (parsed as Record<string, number>) : {};
  } catch {
    return {};
  }
}

/** Atomically write the backfill mtime cache. */
function writeCache(dir: string, cache: Record<string, number>): void {
  mkdirSync(dir, { recursive: true });
  const tmp = join(dir, `.backfill-cache.json.${process.pid}.tmp`);
  writeFileSync(tmp, JSON.stringify(cache), "utf8");
  renameSync(tmp, join(dir, ".backfill-cache.json"));
}

/** Rebuild index.json from every session log present in the store, newest first. */
function rebuildIndex(project: DiscoveredProject): void {
  const dir = projectDir(project.id);
  let entries: string[];
  try {
    entries = readdirSync(dir);
  } catch {
    return;
  }
  const sessions = [];
  for (const name of entries) {
    if (!name.endsWith(".jsonl") || name.startsWith(".")) continue;
    const sessionId = name.replace(/\.jsonl$/, "");
    sessions.push(summarizeSession(sessionId, readSession(project.id, sessionId)));
  }
  sessions.sort((a, b) =>
    (b.endedAt ?? b.startedAt ?? "").localeCompare(a.endedAt ?? a.startedAt ?? ""),
  );
  writeIndex(project.id, { sessions });
}

/** One-shot, current-repo-only transcript backfill into the store. Parses each
 *  session under `<projectsRoot>/<slugFor(project.root)>/`, persists its
 *  meta/spawn/result records (shaped for #117's buildTree), and rebuilds
 *  index.json. An mtime cache makes a 2nd run a no-op (idempotent by sessionId;
 *  the log is rewritten, never duplicated). Returns the session ids (re)written. */
export function backfillRepo(project: DiscoveredProject, opts?: BackfillOptions): string[] {
  const root = opts?.projectsRoot ?? projectsRootDir();
  const slug = join(root, slugFor(project.root));
  const discovered = discoverSessions(slug);
  if (discovered.length === 0) return [];

  const dir = projectDir(project.id);
  const cache = readCache(dir);
  const written: string[] = [];
  let changed = false;

  for (const session of discovered) {
    const logPath = join(dir, `${session.sessionId}.jsonl`);
    if (cache[session.sessionId] === session.mtimeMs && existsSync(logPath)) continue; // idempotent.

    const metas = readMetas(session.subagentsDir);
    const { spawns, results } = gather(session);
    const data: SessionData = { metas, spawns, results };
    const recs = recordsFor(data);
    if (recs.length > 0) {
      rewriteSessionLog(dir, session.sessionId, recs);
      written.push(session.sessionId);
    }
    cache[session.sessionId] = session.mtimeMs;
    changed = true;
  }

  if (changed) {
    rebuildIndex(project);
    writeCache(dir, cache);
  }
  return written;
}
