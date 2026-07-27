// Tolerant Claude Code transcript reader + Agent/Task spawn↔result pairing.
// Transcripts are JSONL; a live one may be mid-append, so the reader parses
// line-by-line and skips any unparseable line (e.g. a truncated trailing frame).
// "Running" is encoded as a spawn with no matching tool_result.

import { readFileSync } from "node:fs";

import { durationMs } from "./activity/tree-builder.ts";

/** Spawn metadata pulled from an `Agent`/`Task` tool_use entry. */
export interface SpawnInfo {
  name: string; // "Agent" | "Task"
  description: string;
  subagentType: string | null;
  startedAt: string | null; // the spawning line's timestamp
}

/** Completion metadata pulled from a matching tool_result entry. */
export interface ResultInfo {
  endedAt: string | null;
  isError: boolean;
}

/** A spawn joined to its result (endedAt null ⟹ still running). */
export interface PairedAgent extends SpawnInfo {
  toolUseId: string;
  endedAt: string | null;
  isError: boolean;
  durationMs: number | null;
}

/** Read a JSONL transcript into objects, skipping blank/unparseable lines. Never throws. */
export function readJsonl(path: string): Record<string, unknown>[] {
  let raw: string;
  try {
    raw = readFileSync(path, "utf8");
  } catch {
    return [];
  }
  const out: Record<string, unknown>[] = [];
  let unparseable = 0;
  let lastParseError = "";
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (trimmed.length === 0) continue;
    try {
      const obj = JSON.parse(trimmed);
      if (obj !== null && typeof obj === "object" && !Array.isArray(obj)) {
        out.push(obj as Record<string, unknown>);
      }
    } catch (err) {
      unparseable++;
      lastParseError = String(err);
    }
  }
  // Exactly one unparseable line is the documented-normal case: a live transcript
  // is read mid-append, so its tail is a half-written line. More than one means
  // real corruption somewhere in the body — surface it once per read rather than
  // once per line, which would spam every poll.
  if (unparseable > 1) {
    console.error(`dashboard: ${unparseable} unparseable lines in ${path} — skipped (last: ${lastParseError})`);
  }
  return out;
}

/** The `content[]` array of a transcript line's message, or [] if absent. */
function contentOf(line: Record<string, unknown>): Record<string, unknown>[] {
  const msg = line.message;
  if (msg === null || typeof msg !== "object") return [];
  const content = (msg as Record<string, unknown>).content;
  if (!Array.isArray(content)) return [];
  return content.filter((c): c is Record<string, unknown> => c !== null && typeof c === "object");
}

function str(v: unknown): string | null {
  return typeof v === "string" && v.length > 0 ? v : null;
}

/** Map of toolUseId → spawn info for every Agent/Task tool_use in the lines. */
export function extractSpawns(lines: Record<string, unknown>[]): Map<string, SpawnInfo> {
  const out = new Map<string, SpawnInfo>();
  for (const line of lines) {
    if (line.type !== "assistant") continue;
    const ts = str(line.timestamp);
    for (const c of contentOf(line)) {
      if (c.type !== "tool_use") continue;
      const name = str(c.name);
      if (name !== "Agent" && name !== "Task") continue;
      const id = str(c.id);
      if (!id || out.has(id)) continue;
      const input = (c.input ?? {}) as Record<string, unknown>;
      out.set(id, {
        name,
        description: str(input.description) ?? "",
        subagentType: str(input.subagent_type),
        startedAt: ts,
      });
    }
  }
  return out;
}

/** Map of toolUseId → result info for every tool_result in the lines (first wins). */
export function extractResults(lines: Record<string, unknown>[]): Map<string, ResultInfo> {
  const out = new Map<string, ResultInfo>();
  for (const line of lines) {
    if (line.type !== "user") continue;
    const ts = str(line.timestamp);
    for (const c of contentOf(line)) {
      if (c.type !== "tool_result") continue;
      const id = str(c.tool_use_id);
      if (!id || out.has(id)) continue;
      out.set(id, { endedAt: ts, isError: c.is_error === true });
    }
  }
  return out;
}

/** Join spawns to results found in the same line set. Unpaired spawns stay running. */
export function pairAgents(lines: Record<string, unknown>[]): PairedAgent[] {
  const spawns = extractSpawns(lines);
  const results = extractResults(lines);
  const out: PairedAgent[] = [];
  for (const [toolUseId, spawn] of spawns) {
    const result = results.get(toolUseId);
    out.push({
      ...spawn,
      toolUseId,
      endedAt: result?.endedAt ?? null,
      isError: result?.isError ?? false,
      durationMs: durationMs(spawn.startedAt, result?.endedAt ?? null),
    });
  }
  return out;
}
