/**
 * The OpenCode status record (#359). The adapter's readiness lives in its
 * plugin process, out of reach of a skill's bash call, so every settled
 * startup leaves one bounded record at `<data root>/toolu/opencode-status.json`.
 * The statusline status helper reads it through `TOOLU_CONFIG_DIR`, which
 * `shell.env` sets to that data root. The record is diagnostic only: nothing
 * enforces from it.
 *
 * No schema library: the statusline bundle carries this reader. Validation is
 * strict all the same: unknown keys, a wrong version or a wrong type make the
 * whole record invalid.
 */
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";

export const OPENCODE_STATUS_FILE = "opencode-status.json";

/** A record is a few KB; anything this size (256 KiB) is not one. */
export const MAX_OPENCODE_STATUS_BYTES = 262_144;

export type OpencodeStatusPlugin = { name: string; entries: string[]; artifacts: number };

export type OpencodeStatusRecord = {
  version: 1;
  /** ISO-8601 time of the write. */
  written: string;
  /** The OpenCode instance's project root. */
  project: string;
  status: "ready" | "not-ready";
  /** Why startup is not ready; not-ready only. */
  reason?: string;
  /** Where the plugin selection came from; ready only. */
  selection?: "project" | "global" | "default";
  /** Each selected plugin's startup, in startup order; empty when not ready. */
  plugins: OpencodeStatusPlugin[];
  /** Non-fatal startup notes, bounded by the writer. */
  notes: string[];
};

export type OpencodeStatusRead =
  | { ok: true; record: OpencodeStatusRecord }
  | { ok: false; reason: "missing" | "invalid" };

/** `<config root>/toolu/opencode-status.json`; on OpenCode the config root is the data root. */
export function opencodeStatusPath(configRoot: string): string {
  return join(configRoot, "toolu", OPENCODE_STATUS_FILE);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// Key lists stay inside functions: every SessionStart bundle imports this module's
// barrel, and a top-level value would be carried into each of them.
function onlyKeys(value: Record<string, unknown>, allowed: readonly string[]): boolean {
  return Object.keys(value).every((key) => allowed.includes(key));
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isPlugin(value: unknown): value is OpencodeStatusPlugin {
  return (
    isObject(value) &&
    onlyKeys(value, ["name", "entries", "artifacts"]) &&
    typeof value["name"] === "string" &&
    value["name"] !== "" &&
    isStringArray(value["entries"]) &&
    Number.isInteger(value["artifacts"]) &&
    Number(value["artifacts"]) >= 0
  );
}

function isSelection(value: unknown): value is OpencodeStatusRecord["selection"] {
  return value === undefined || value === "project" || value === "global" || value === "default";
}

/** `value` when it is exactly a version 1 record, else undefined. */
export function parseOpencodeStatus(value: unknown): OpencodeStatusRecord | undefined {
  const keys = [
    "version",
    "written",
    "project",
    "status",
    "reason",
    "selection",
    "plugins",
    "notes",
  ];
  if (!isObject(value) || !onlyKeys(value, keys)) return undefined;
  const { version, written, project, status, reason, selection, plugins, notes } = value;
  if (version !== 1 || typeof written !== "string" || typeof project !== "string") return undefined;
  if (status !== "ready" && status !== "not-ready") return undefined;
  if (reason !== undefined && typeof reason !== "string") return undefined;
  if (!isSelection(selection) || !Array.isArray(plugins) || !isStringArray(notes)) return undefined;
  if (!plugins.every(isPlugin)) return undefined;
  const record: OpencodeStatusRecord = { version, written, project, status, plugins, notes };
  if (reason !== undefined) record.reason = reason;
  if (selection !== undefined) record.selection = selection;
  return record;
}

function errorCode(error: unknown): string | undefined {
  return error instanceof Error && "code" in error && typeof error.code === "string"
    ? error.code
    : undefined;
}

/** The record at `path`: missing when absent, invalid when unreadable, oversized or malformed. */
export function readOpencodeStatus(path: string): OpencodeStatusRead {
  let text: string;
  try {
    if (statSync(path).size > MAX_OPENCODE_STATUS_BYTES) return { ok: false, reason: "invalid" };
    text = readFileSync(path, "utf8");
  } catch (error) {
    const code = errorCode(error);
    return { ok: false, reason: code === "ENOENT" || code === "ENOTDIR" ? "missing" : "invalid" };
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { ok: false, reason: "invalid" };
  }
  const record = parseOpencodeStatus(parsed);
  return record === undefined ? { ok: false, reason: "invalid" } : { ok: true, record };
}
