// Persistent per-repo activity store, keyed by #117's discovery.ts projectId
// (sha1(`${root}@${branchSlug}`).slice(0,12)). Layout under the base dir (env
// TOOLU_ACTIVITY_DIR, default `${CLAUDE_CONFIG_DIR:-~/.claude}/toolu/activity`):
//   <projectId>/<sessionId>.jsonl — append-only NDJSON of ActivityRecords
//   <projectId>/index.json        — { sessions: SessionSummary[] }
// Records are SHAPED so #117's tree-builder renders them (kind-tagged AgentMeta /
// SpawnRec / ResultRec). readSession reassembles {metas, spawns, results} for
// buildTree, tolerating a torn trailing NDJSON line (skip, never throw). appendRecord
// does a single atomic O_APPEND line; applyRetention drops aged / over-cap logs and
// rewrites index.json atomically. No reader throws on a missing/corrupt store; every
// caller-supplied id is assertSafeId-validated (path-traversal defence).

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

import { loadConfig } from "../config.ts";
import { type AgentMeta, type ResultRec, type SpawnRec } from "./tree-builder.ts";

/** Allowlist for a projectId/sessionId path segment: leading alphanumeric, then
 *  only `[A-Za-z0-9._-]`. A 12-hex projectId and a uuid/hex-dash sessionId match. */
const SAFE_ID = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

/** Reject any id that could escape the store when joined into a path. Throws a
 *  clear Error for empty, separator/NUL-bearing, `.`/`..`, `..`-containing, or
 *  non-allowlisted ids — the single path-traversal chokepoint for every store fs
 *  path derived from a caller-supplied projectId/sessionId. */
export function assertSafeId(id: string): string {
  const reject = (why: string): never => {
    throw new Error(`activity store: unsafe id (${why}): ${JSON.stringify(id)}`);
  };
  if (typeof id !== "string" || id.length === 0) reject("empty");
  if (id.includes("/") || id.includes("\\") || id.includes("\0")) reject("path separator or NUL");
  if (id === "." || id === ".." || id.includes("..")) reject("resolves outside the store");
  // A trailing dot is stripped by some filesystems (NTFS), which would let two
  // distinct ids collide on one directory — no real projectId/sessionId ends in one.
  if (id.endsWith(".")) reject("trailing dot");
  if (!SAFE_ID.test(id)) reject("not in allowlist [A-Za-z0-9][A-Za-z0-9._-]*");
  return id;
}

/** A persisted meta record: an agent's identity (kind-tagged AgentMeta). */
export interface MetaRecord extends AgentMeta {
  kind: "meta";
}

/** A persisted spawn record: a tool_use start, keyed by toolUseId (kind-tagged SpawnRec). */
export interface SpawnRecord extends SpawnRec {
  kind: "spawn";
  toolUseId: string; // join key to its meta + result
}

/** A persisted result record: a tool_result, keyed by toolUseId (kind-tagged ResultRec). */
export interface ResultRecord extends ResultRec {
  kind: "result";
  toolUseId: string; // join key to its spawn + meta
}

/** One line in a session log. The discriminated union the store reads/writes. */
export type ActivityRecord = MetaRecord | SpawnRecord | ResultRecord;

/** Per-session rollup stored in index.json (the history-lane session list). */
export interface SessionSummary {
  sessionId: string;
  startedAt: string | null; // earliest spawn timestamp, or null if none recorded
  endedAt: string | null; // latest result timestamp, or null while any agent runs
  agentCount: number; // number of agents (meta records) in the session
  errored: boolean; // true if any recorded result carried is_error
  running: boolean; // true if any spawn has no matching result (an agent still running)
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

/** The `<base>/<projectId>` directory for one project (projectId is assertSafeId-validated). */
export function projectDir(projectId: string): string {
  return join(activityBaseDir(), assertSafeId(projectId));
}

/** The append-only NDJSON log path for one session (both ids are assertSafeId-validated). */
function sessionLogPath(projectId: string, sessionId: string): string {
  return join(projectDir(projectId), `${assertSafeId(sessionId)}.jsonl`);
}

/** The configured prompt-preview cap, read once and memoised (default 120). */
let cachedPreviewChars: number | undefined;
function promptPreviewChars(): number {
  if (cachedPreviewChars === undefined) cachedPreviewChars = loadConfig().config.promptPreviewChars;
  return cachedPreviewChars;
}

/** The single store-side truncation chokepoint: bound a record's preview field
 *  (`description`) to promptPreviewChars before persisting, so no writer ever stores
 *  the full prompt. Returns a new record; non-meta kinds pass through. */
export function truncateRecord(rec: ActivityRecord, max: number = promptPreviewChars()): ActivityRecord {
  if (rec.kind !== "meta") return rec;
  const d = rec.description;
  return d.length > max ? { ...rec, description: d.slice(0, max) } : rec;
}

/** Append a record as one atomic O_APPEND NDJSON line (creates the project dir on
 *  first write). The preview field is truncated to promptPreviewChars first, so no
 *  writer persists the full prompt. Each line is one write() to an O_APPEND fd, so
 *  concurrent appends never interleave. */
export function appendRecord(projectId: string, sessionId: string, rec: ActivityRecord): void {
  const dir = projectDir(projectId);
  mkdirSync(dir, { recursive: true });
  const line = `${JSON.stringify(truncateRecord(rec))}\n`;
  // O_APPEND guarantees the kernel seeks to EOF atomically per write().
  const fd = openSync(sessionLogPath(projectId, sessionId), fsConstants.O_WRONLY | fsConstants.O_CREAT | fsConstants.O_APPEND, 0o644);
  try {
    writeSync(fd, line);
  } finally {
    closeSync(fd);
  }
}

/** True for a Node fs "file not found" error — the store's normal "nothing
 *  persisted yet" case, distinguished from real I/O errors worth surfacing.
 *  `in`-narrows without an `as` assertion. */
export function isEnoent(err: unknown): boolean {
  return typeof err === "object" && err !== null && "code" in err && err.code === "ENOENT";
}

/** Surface an unexpected store error, staying quiet on the ENOENT that just
 *  means "nothing persisted yet". The tolerant readers below degrade to an empty
 *  fallback on any fs failure; routing that failure through here keeps a real
 *  I/O fault (permissions, corruption) from vanishing silently. */
function logStoreError(context: string, err: unknown): void {
  if (!isEnoent(err)) console.error(`activity store: ${context} — ${String(err)}`);
}

/** Runtime shape check for a parsed index.json — the store treats anything that
 *  fails as absent. A type guard (not an `as` cast): validates the one field the
 *  readers rely on, `sessions` being an array. */
function isProjectIndex(v: unknown): v is ProjectIndex {
  return typeof v === "object" && v !== null && "sessions" in v && Array.isArray(v.sessions);
}

/** Runtime shape check for one parsed session-log line: an object tagged with a
 *  known `kind`. A type guard (not an `as` cast); the per-kind field access
 *  below stays defensively guarded, so this only needs the discriminant. */
function isActivityRecord(v: unknown): v is ActivityRecord {
  return (
    typeof v === "object" &&
    v !== null &&
    "kind" in v &&
    (v.kind === "meta" || v.kind === "spawn" || v.kind === "result")
  );
}

/** Read and parse `<projectId>/index.json`, or null when absent/corrupt. */
function readIndex(projectId: string): ProjectIndex | null {
  let raw: string;
  try {
    raw = readFileSync(join(projectDir(projectId), "index.json"), "utf8");
  } catch (err) {
    // A missing index is the normal "no history yet" state (hit on every tick),
    // so stay quiet on ENOENT; surface anything else (permissions, I/O) before
    // treating the project as having no sessions.
    if (!isEnoent(err)) {
      console.error(`activity store: reading index for ${projectId} failed — treating as absent (${String(err)})`);
    }
    return null;
  }
  if (raw.trim().length === 0) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return isProjectIndex(parsed) ? parsed : null;
  } catch (err) {
    // The file existed and read cleanly, so a throw here is corrupt JSON — worth
    // surfacing. Still treated as absent, never fatal.
    console.error(`activity store: index for ${projectId} is corrupt JSON — treating as absent (${String(err)})`);
    return null;
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
 *  buildTree consumes. A torn/unparseable line is SKIPPED (never thrown); empty maps
 *  when the log is absent; a spawn/result without a toolUseId is ignored. A malformed
 *  (path-traversal) id THROWS — validation runs before the fs read (hard error, not
 *  the silently-empty read a genuinely missing file yields). */
export function readSession(projectId: string, sessionId: string): SessionData {
  const metas: AgentMeta[] = [];
  const spawns = new Map<string, SpawnRec>();
  const results = new Map<string, ResultRec>();
  const logPath = sessionLogPath(projectId, sessionId); // validates ids before the read
  let raw: string;
  try {
    raw = readFileSync(logPath, "utf8");
  } catch (err) {
    logStoreError(`reading session log ${sessionId}`, err);
    return { metas, spawns, results };
  }
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length === 0) continue;
    let parsed: unknown;
    try {
      parsed = JSON.parse(trimmed);
    } catch {
      continue; // torn/partial line: skip it, keep going.
    }
    if (!isActivityRecord(parsed)) continue;
    const rec = parsed;
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
    if (!data.results.has(toolUseId)) { running = true; break; }
  }
  return { sessionId, startedAt, endedAt: running ? null : endedAt, agentCount: data.metas.length, errored, running };
}

/** A session's representative timestamp for ordering: end if present, else start. */
function sessionTime(s: SessionSummary): string {
  return s.endedAt ?? s.startedAt ?? "1970-01-01T00:00:00.000Z";
}

/** Retention options; default to the loaded dashboard config when omitted. */
export interface RetentionOptions {
  retentionDays: number; // drop sessions older than this many days
  maxSessionsPerProject: number; // keep at most this many sessions (newest first)
}

/** Drop session logs older than retentionDays OR beyond maxSessionsPerProject
 *  (newest kept), delete their NDJSON files, and rewrite index.json atomically.
 *  No-op when the index is absent. Returns the removed session ids.
 *
 *  @param nowMs - the clock the age cutoff is measured from (default Date.now()).
 *  Callers that already have an injected clock — the aggregate lane threads one
 *  through every other read — must pass it, so eviction is reproducible rather
 *  than wall-clock dependent. */
export function applyRetention(projectId: string, opts: RetentionOptions, nowMs: number = Date.now()): string[] {
  const index = readIndex(projectId);
  if (!index) return [];
  const cutoffMs = nowMs - opts.retentionDays * 24 * 60 * 60 * 1000;
  // Newest-first ordering for both the age cutoff and the count cap.
  const ranked = [...index.sessions].sort((a, b) => sessionTime(b).localeCompare(sessionTime(a)));
  const kept: SessionSummary[] = [];
  const removed: string[] = [];
  for (let i = 0; i < ranked.length; i++) {
    const s = ranked[i];
    if (Date.parse(sessionTime(s)) < cutoffMs || i >= opts.maxSessionsPerProject) removed.push(s.sessionId);
    else kept.push(s);
  }
  if (removed.length === 0) return [];
  for (const sessionId of removed) {
    try {
      rmSync(sessionLogPath(projectId, sessionId), { force: true }); // best-effort; index is the truth
    } catch {
      /* index rewrite below is the source of truth */
    }
  }
  writeIndex(projectId, { sessions: kept });
  return removed;
}

/** True when a project's store dir exists (helper for callers/tests). */
export function storeHasProject(projectId: string): boolean {
  try {
    return statSync(projectDir(projectId)).isDirectory();
  } catch (err) {
    logStoreError(`stat project ${projectId}`, err);
    return false;
  }
}

/** List the session ids whose `.jsonl` logs exist on disk (no index needed). */
export function listSessionFiles(projectId: string): string[] {
  let entries: string[];
  try {
    entries = readdirSync(projectDir(projectId));
  } catch (err) {
    logStoreError(`listing session files for ${projectId}`, err);
    return [];
  }
  return entries
    .filter((f) => f.endsWith(".jsonl") && !f.startsWith("."))
    .map((f) => f.replace(/\.jsonl$/, ""));
}
