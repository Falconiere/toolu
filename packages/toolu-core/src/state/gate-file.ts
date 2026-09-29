/**
 * Multi-slot quality-gate file (#255), a port of `gate-file.sh`. Every
 * failing file owns an entry under `entries`, keyed by its path. The top-level
 * fields mirror the most recent failure and concatenate every open
 * violation, so readers keep the `.status`/`.reason`/`.violations` contract.
 * Output bytes equal the jq programs' for the same inputs.
 *
 * Where it goes past bash:
 * - Writes run under `<gate>.lock`, so concurrent TypeScript writers never
 *   drop each other's entries.
 * - A document that parses but fails the strict v1 schema is "unrecognized".
 *   Recording replaces it, with a warning and a `.dropped.log` line (fail
 *   closed: the new failure always lands). Clearing leaves it alone.
 */
import { appendFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import {
  compareJqStrings,
  isoSeconds,
  stderrWarn,
  toJqJson,
  withLock,
  writeAtomic,
  type StateOptions,
  type Warn,
} from "./state-io.ts";
import { GateFileSchema, type GateEntries, type GateEntry, type GateFile } from "./state-schema.ts";
import { telemetryAppend } from "./telemetry.ts";

export const GLOBAL_GATE_KEY = "__global__";

export type GateRead =
  | { kind: "missing" }
  | { kind: "malformed"; reason: string }
  | { kind: "unrecognized"; reason: string; value: unknown }
  | { kind: "ok"; doc: GateFile };

function isGateFile(value: unknown): value is GateFile {
  return GateFileSchema.safeParse(value).success;
}

function firstIssue(value: unknown): string {
  const parsed = GateFileSchema.safeParse(value);
  const issue = parsed.success ? undefined : parsed.error.issues[0];
  return issue === undefined ? "invalid" : `${issue.path.join(".") || "(root)"}: ${issue.message}`;
}

/**
 * Read and classify the gate file. Empty content, `null` and `false` count as
 * malformed, as with `jq -e .`. A valid document is returned as parsed (not
 * zod's copy), so every key keeps its position in the file.
 */
export function readGateFile(gateFile: string): GateRead {
  if (!existsSync(gateFile)) return { kind: "missing" };
  let value: unknown;
  try {
    value = JSON.parse(readFileSync(gateFile, "utf8"));
  } catch (error) {
    return { kind: "malformed", reason: String(error) };
  }
  if (value === null || value === false) return { kind: "malformed", reason: String(value) };
  if (isGateFile(value)) return { kind: "ok", doc: value };
  return { kind: "unrecognized", reason: firstIssue(value), value };
}

/** The seed both jq programs start with: `entries`, else the legacy single slot, else nothing. */
function seedEntries(doc: GateFile): GateEntries {
  if (doc.status !== "failing") return {};
  if (doc.entries !== undefined) return doc.entries;
  const { source, reason, violations, updatedAt } = doc;
  return { [doc.file]: { source, reason, violations, updatedAt } };
}

/** Oldest first by (updatedAt, key): jq's `sort_by(.value.updatedAt // "", .key)`. */
function sortedEntries(entries: GateEntries): [string, GateEntry][] {
  return Object.entries(entries).toSorted(
    ([ka, a], [kb, b]) => compareJqStrings(a.updatedAt, b.updatedAt) || compareJqStrings(ka, kb),
  );
}

function joinViolations(sorted: [string, GateEntry][]): string {
  return sorted.map(([, entry]) => entry.violations).join("");
}

/** The root that owns `<root>/<host dir>/tmp/quality-gate-status.json`: three hops up. */
function gateRoot(gateFile: string): string {
  return dirname(dirname(dirname(gateFile)));
}

/** Entries a replaced document held besides `file`, counted with the seed rule. */
function droppedCount(value: unknown, file: string): number {
  if (!isJsonObject(value)) return 0;
  let keys: string[] = [];
  if (isJsonObject(value.entries)) {
    keys = Object.keys(value.entries);
  } else if (value.status === "failing") {
    keys = [typeof value.file === "string" ? value.file : GLOBAL_GATE_KEY];
  }
  return keys.filter((key) => key !== file).length;
}

/** Durable breadcrumb beside the gate file. Best effort, as in bash: logging never blocks the write. */
function breadcrumb(gateFile: string, line: string): void {
  try {
    appendFileSync(`${gateFile}.dropped.log`, `${line}\n`);
  } catch {
    // A failed breadcrumb costs only the log line.
  }
}

type Failure = { file: string; source: string; reason: string; violations: string; now: string };

function failingDoc(prev: GateEntries, f: Failure): GateFile {
  const entry = { source: f.source, reason: f.reason, violations: f.violations, updatedAt: f.now };
  const entries = { ...prev, [f.file]: entry };
  return {
    status: "failing",
    reason: f.reason,
    source: f.source,
    file: f.file,
    violations: joinViolations(sortedEntries(entries)),
    entries,
    updatedAt: f.now,
  };
}

/** Bash's fallback when the primary write fails: a single-slot record, so this failure is never lost. */
function writeSingleSlot(gateFile: string, previous: unknown, f: Failure, warn: Warn): void {
  const dropped = droppedCount(previous, f.file);
  if (dropped > 0) {
    warn(
      `gate-file: primary write failed at ${gateFile}; single-slot fallback dropped ${String(dropped)} other entry(ies)`,
    );
    breadcrumb(
      gateFile,
      `${f.now} primary write failed; single-slot fallback dropped ${String(dropped)} entry(ies)`,
    );
  }
  const { reason, source, file, violations, now } = f;
  const doc = { status: "failing", reason, source, file, violations, updatedAt: now };
  try {
    writeFileSync(gateFile, `${toJqJson(doc, true)}\n`);
  } catch {
    // bash `> "$gate_file" || true`: nothing further can record it.
  }
}

/** `gate_record_failure GATE_FILE FILE SOURCE REASON VIOLATIONS`: add or replace FILE's entry. */
export function recordGateFailure(
  gateFile: string,
  file: string,
  source: string,
  reason: string,
  violations: string,
  options: StateOptions = {},
): void {
  const warn = options.warn ?? stderrWarn;
  const f = { file, source, reason, violations, now: isoSeconds(options.now?.() ?? new Date()) };
  withLock(
    gateFile,
    () => {
      const existing = readGateFile(gateFile);
      let prev: GateEntries = {};
      let previous: unknown = {};
      if (existing.kind === "ok") {
        prev = seedEntries(existing.doc);
        previous = existing.doc;
      } else if (existing.kind === "unrecognized") {
        previous = existing.value;
        const dropped = droppedCount(existing.value, file);
        warn(`gate-file: unrecognized gate file at ${gateFile} (${existing.reason}); replacing it`);
        breadcrumb(
          gateFile,
          `${f.now} unrecognized gate file replaced; dropped ${String(dropped)} entry(ies)`,
        );
      }
      if (!writeAtomic(gateFile, `${toJqJson(failingDoc(prev, f), true)}\n`)) {
        writeSingleSlot(gateFile, previous, f, warn);
      }
    },
    { warn },
  );
  // One event per recorded failure, re-records included (bash semantics).
  telemetryAppend(gateRoot(gateFile), "gate_fail", { file, source }, options);
}

function owns(doc: GateFile & { status: "failing" }, file: string, source: string): boolean {
  if (doc.entries === undefined) return doc.source === source && doc.file === file;
  const entry = Object.hasOwn(doc.entries, file) ? doc.entries[file] : undefined;
  return (entry?.source ?? "") === source;
}

function clearedDoc(left: GateEntries, source: string, now: string): GateFile {
  const sorted = sortedEntries(left);
  const latest = sorted.at(-1);
  if (latest === undefined) return { status: "passing", source, updatedAt: now };
  const [key, entry] = latest;
  return {
    status: "failing",
    reason: entry.reason,
    source: entry.source,
    file: key,
    violations: joinViolations(sorted),
    entries: left,
    updatedAt: now,
  };
}

function clearUnderLock(
  gateFile: string,
  file: string,
  source: string,
  now: string,
  warn: Warn,
): boolean {
  const existing = readGateFile(gateFile);
  if (existing.kind === "malformed") {
    warn(
      `gate-file: malformed JSON at ${gateFile}; ignoring clear (gate stays failing until next write)`,
    );
  } else if (existing.kind === "unrecognized") {
    warn(`gate-file: unrecognized gate file at ${gateFile} (${existing.reason}); ignoring clear`);
  }
  if (existing.kind !== "ok") return false;
  const doc = existing.doc;
  if (doc.status !== "failing" || !owns(doc, file, source)) return false;
  const left = { ...seedEntries(doc) };
  delete left[file];
  return writeAtomic(gateFile, `${toJqJson(clearedDoc(left, source, now), true)}\n`);
}

/**
 * `gate_clear_file GATE_FILE FILE SOURCE`: drop FILE's entry if SOURCE owns it.
 * The latest remaining entry is promoted; the file reads "passing" only when
 * none remain. `gate_clear` telemetry fires only after a write lands.
 */
export function clearGateFile(
  gateFile: string,
  file: string,
  source: string,
  options: StateOptions = {},
): "cleared" | "noop" {
  const warn = options.warn ?? stderrWarn;
  const now = isoSeconds(options.now?.() ?? new Date());
  const cleared = withLock(gateFile, () => clearUnderLock(gateFile, file, source, now, warn), {
    warn,
  });
  if (!cleared) return "noop";
  telemetryAppend(gateRoot(gateFile), "gate_clear", { file, source }, options);
  return "cleared";
}
