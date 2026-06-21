// Persistent per-repo activity store, keyed by #117's discovery.ts projectId
// (sha1(`${root}@${branchSlug}`).slice(0,12)). Layout under the base dir
// (env TOOLU_ACTIVITY_DIR, default `${CLAUDE_CONFIG_DIR:-~/.claude}/toolu/activity`):
//   <projectId>/<sessionId>.jsonl — append-only NDJSON of ActivityRecords
//   <projectId>/index.json        — { sessions: SessionSummary[] }
// Records are SHAPED so #117's tree-builder renders them: each carries exactly the
// fields AgentMeta / SpawnRec / ResultRec need, tagged by a `kind` discriminator.
// readSession reassembles {metas, spawns, results} ready for buildTree(...) and
// tolerates a torn trailing NDJSON line (skip, never throw). appendRecord does a
// single atomic O_APPEND line. applyRetention drops aged / over-cap session logs
// and rewrites index.json atomically. No reader throws on a missing/corrupt store.

import {
  closeSync,
  constants as fsConstants,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
  writeSync,
} from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import { type AgentMeta, type ResultRec, type SpawnRec } from "./tree-builder.ts";

/** A persisted meta record: an agent's identity (kind-tagged AgentMeta). */
export interface MetaRecord extends AgentMeta {
  kind: "meta";
}

/** A persisted spawn record: a tool_use start, keyed by toolUseId (kind-tagged SpawnRec). */
export interface SpawnRecord extends SpawnRec {
  kind: "spawn";
  /** The tool_use id this spawn belongs to (join key to its meta + result). */
  toolUseId: string;
}

/** A persisted result record: a tool_result, keyed by toolUseId (kind-tagged ResultRec). */
export interface ResultRecord extends ResultRec {
  kind: "result";
  /** The tool_use id this result completes (join key to its spawn + meta). */
  toolUseId: string;
}

/** One line in a session log. The discriminated union the store reads/writes. */
export type ActivityRecord = MetaRecord | SpawnRecord | ResultRecord;

/** Per-session rollup stored in index.json (the history-lane session list). */
export interface SessionSummary {
  sessionId: string;
  /** Earliest spawn timestamp seen, or null if none recorded. */
  startedAt: string | null;
  /** Latest result timestamp seen, or null if any agent is still running. */
  endedAt: string | null;
  /** Number of agents (meta records) in the session. */
  agentCount: number;
  /** True if any recorded result carried is_error. */
  errored: boolean;
  /** True if any spawn has no matching result (an agent still running). */
  running: boolean;
}

/** The {metas, spawns, results} bundle #117's buildTree consumes. */
export interface SessionData {
  metas: AgentMeta[];
  spawns: Map<string, SpawnRec>;
  results: Map<string, ResultRec>;
}

/** On-disk per-project index: just the session summaries (newest first). */
export interface ProjectIndex {
  sessions: SessionSummary[];
}

/** Resolve the store base dir: $TOOLU_ACTIVITY_DIR, else
 *  `${CLAUDE_CONFIG_DIR:-~/.claude}/toolu/activity`. Tests point the env at temp. */
export function activityBaseDir(): string {
  const env = process.env.TOOLU_ACTIVITY_DIR;
  if (env && env.length > 0) return env;
  const cfgDir = process.env.CLAUDE_CONFIG_DIR;
  const base = cfgDir && cfgDir.length > 0 ? cfgDir : join(process.env.HOME ?? homedir(), ".claude");
  return join(base, "toolu", "activity");
}

/** The `<base>/<projectId>` directory for one project. */
export function projectDir(projectId: string): string {
  return join(activityBaseDir(), projectId);
}

/** The append-only NDJSON log path for one session. */
function sessionLogPath(projectId: string, sessionId: string): string {
  return join(projectDir(projectId), `${sessionId}.jsonl`);
}

/** Append a single record as one atomic O_APPEND NDJSON line. Creates the
 *  project dir on first write. Concurrent appenders are safe: each line is one
 *  write() to an O_APPEND fd, so lines never interleave. */
export function appendRecord(projectId: string, sessionId: string, rec: ActivityRecord): void {
  const dir = projectDir(projectId);
  mkdirSync(dir, { recursive: true });
  const line = `${JSON.stringify(rec)}\n`;
  // O_APPEND guarantees the kernel seeks to EOF atomically per write().
  const fd = openSync(sessionLogPath(projectId, sessionId), fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_APPEND, 0o644);
  try {
    writeSync(fd, line);
  } finally {
    closeSync(fd);
  }
}

/** Read and parse `<projectId>/index.json`, or null when absent/corrupt. */
function readIndex(projectId: string): ProjectIndex | null {
  let raw: string;
  try {
    raw = readFileSync(join(projectDir(projectId), "index.json"), "utf8");
  } catch {
    return null;
  }
  if (raw.trim().length === 0) return null;
  try {
    const parsed = JSON.parse(raw) as ProjectIndex;
    if (!parsed || typeof parsed !== "object" || !Array.isArray(parsed.sessions)) return null;
    return parsed;
  } catch {
    return null; // corrupt index: treated as absent, never fatal.
  }
}

/** Atomically write `index` to `<projectId>/index.json` via temp file + rename. */
export function writeIndex(projectId: string, index: ProjectIndex): void {
  const dir = projectDir(projectId);
  mkdirSync(dir, { recursive: true });
  const tmpPath = join(dir, `.index.json.${process.pid}.tmp`);
  writeFileSync(tmpPath, `${JSON.stringify(index, null, 2)}\n`, "utf8");
  renameSync(tmpPath, join(dir, "index.json")); // atomic on the same filesystem.
}

/** Session summaries for one project, from its index.json (no fs scan). [] if absent. */
export function listSessions(projectId: string): SessionSummary[] {
  return readIndex(projectId)?.sessions ?? [];
}

/** Read a session log tolerantly into the {metas, spawns, results} bundle #117's
 *  buildTree consumes. Each NDJSON line is parsed; a torn/partial/unparseable line
 *  (e.g. an interrupted append) is SKIPPED, never thrown. Returns empty maps when
 *  the log is absent. A spawn/result without a toolUseId is ignored. */
export function readSession(projectId: string, sessionId: string): SessionData {
  const metas: AgentMeta[] = [];
  const spawns = new Map<string, SpawnRec>();
  const results = new Map<string, ResultRec>();
  let raw: string;
  try {
    raw = readFileSync(sessionLogPath(projectId, sessionId), "utf8");
  } catch {
    return { metas, spawns, results };
  }
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length === 0) continue;
    let rec: ActivityRecord;
    try {
      rec = JSON.parse(trimmed) as ActivityRecord;
    } catch {
      continue; // torn/partial line: skip it, keep going.
    }
    if (!rec || typeof rec !== "object") continue;
    if (rec.kind === "meta") {
      if (typeof rec.toolUseId === "string" && rec.toolUseId.length > 0) metas.push(rec);
    } else if (rec.kind === "spawn") {
      if (typeof rec.toolUseId === "string" && rec.toolUseId.length > 0) {
        spawns.set(rec.toolUseId, { startedAt: rec.startedAt, ownerAgentId: rec.ownerAgentId });
      }
    } else if (rec.kind === "result") {
      // First result for a toolUseId wins, matching transcript.ts/extractResults.
      if (typeof rec.toolUseId === "string" && rec.toolUseId.length > 0 && !results.has(rec.toolUseId)) {
        results.set(rec.toolUseId, { endedAt: rec.endedAt, isError: rec.isError });
      }
    }
  }
  return { metas, spawns, results };
}

/** Build a SessionSummary from a session's reassembled records. */
export function summarizeSession(sessionId: string, data: SessionData): SessionSummary {
  let startedAt: string | null = null;
  let endedAt: string | null = null;
  let errored = false;
  for (const [, s] of data.spawns) {
    if (s.startedAt && (startedAt === null || s.startedAt < startedAt)) startedAt = s.startedAt;
  }
  for (const [, r] of data.results) {
    if (r.isError) errored = true;
    if (r.endedAt && (endedAt === null || r.endedAt > endedAt)) endedAt = r.endedAt;
  }
  // Running = at least one spawn without a matching result.
  let running = false;
  for (const toolUseId of data.spawns.keys()) {
    if (!data.results.has(toolUseId)) {
      running = true;
      break;
    }
  }
  return { sessionId, startedAt, endedAt: running ? null : endedAt, agentCount: data.metas.length, errored, running };
}

/** A session's representative timestamp for ordering: end if present, else start. */
function sessionTime(s: SessionSummary): string {
  return s.endedAt ?? s.startedAt ?? "1970-01-01T00:00:00.000Z";
}

/** Retention options; default to the loaded dashboard config when omitted. */
export interface RetentionOptions {
  /** Drop sessions older than this many days. */
  retentionDays: number;
  /** Keep at most this many sessions (newest first). */
  maxSessionsPerProject: number;
}

/** Drop session logs older than retentionDays OR beyond maxSessionsPerProject
 *  (keeping the newest), delete their NDJSON files, and rewrite index.json
 *  atomically. No-op when the index is absent. Returns the removed session ids. */
export function applyRetention(projectId: string, opts: RetentionOptions): string[] {
  const index = readIndex(projectId);
  if (!index) return [];
  const dir = projectDir(projectId);
  const cutoffMs = Date.now() - opts.retentionDays * 24 * 60 * 60 * 1000;

  // Newest-first ordering for both the age cutoff and the count cap.
  const ranked = [...index.sessions].sort((a, b) => sessionTime(b).localeCompare(sessionTime(a)));
  const kept: SessionSummary[] = [];
  const removed: string[] = [];
  for (let i = 0; i < ranked.length; i++) {
    const s = ranked[i];
    const tooOld = Date.parse(sessionTime(s)) < cutoffMs;
    const overCap = i >= opts.maxSessionsPerProject;
    if (tooOld || overCap) removed.push(s.sessionId);
    else kept.push(s);
  }
  if (removed.length === 0) return [];

  for (const sessionId of removed) {
    try {
      rmSync(sessionLogPath(projectId, sessionId), { force: true });
    } catch {
      // Best-effort delete; the index rewrite below is the source of truth.
    }
  }
  writeIndex(projectId, { sessions: kept });
  return removed;
}

/** True when a project's store dir exists (helper for callers/tests). */
export function storeHasProject(projectId: string): boolean {
  try {
    return statSync(projectDir(projectId)).isDirectory();
  } catch {
    return false;
  }
}

/** List the session ids whose `.jsonl` logs exist on disk (no index needed). */
export function listSessionFiles(projectId: string): string[] {
  let entries: string[];
  try {
    entries = readdirSync(projectDir(projectId));
  } catch {
    return [];
  }
  return entries
    .filter((f) => f.endsWith(".jsonl") && !f.startsWith("."))
    .map((f) => f.replace(/\.jsonl$/, ""));
}
