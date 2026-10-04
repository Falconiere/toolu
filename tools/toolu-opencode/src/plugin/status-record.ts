/**
 * The status a settled startup leaves behind (#359): one bounded record in the
 * project's data root for the statusline status skill, and one structured
 * host-log entry. Both carry only toolu's own verdict, plugin and entry names,
 * counts, paths and startup notes; never a value from the environment.
 */
import { randomUUID } from "node:crypto";
import { mkdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { OpencodeStatusRecord } from "@toolu/core/startup";
import { opencodeDataRoot, opencodeRegistryRoot } from "../host/roots.ts";
import type { HostBinding, LogExtra } from "./context.ts";
import type { Enforcement } from "./enforcement.ts";

/**
 * `OPENCODE_STATUS_FILE` of `@toolu/core/startup`, spelled here: the npm route
 * installs `@toolu/core` from the registry, where the release that adds the
 * export may not be the one installed. A test pins the two together.
 */
export const STATUS_FILE = "opencode-status.json";

const MAX_REASON_CHARS = 4_000;
const MAX_NOTES = 20;
const MAX_NOTE_CHARS = 500;

function bounded(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max)}…` : text;
}

export function statusRecord(
  enforcement: Enforcement,
  project: string,
  now: Date,
): OpencodeStatusRecord {
  const base: Pick<OpencodeStatusRecord, "version" | "written" | "project"> = {
    version: 1,
    written: now.toISOString(),
    project,
  };
  if (enforcement.status !== "ready") {
    const reason = bounded(enforcement.reason, MAX_REASON_CHARS);
    return { ...base, status: "not-ready", reason, plugins: [], notes: [] };
  }
  return {
    ...base,
    status: "ready",
    selection: enforcement.selectionSource,
    plugins: enforcement.plugins.map((plugin) => ({
      name: plugin.plugin,
      entries: plugin.entries.map((entry) => entry.entry),
      artifacts: plugin.artifacts.length,
    })),
    notes: enforcement.diagnostics.slice(0, MAX_NOTES).map((note) => bounded(note, MAX_NOTE_CHARS)),
  };
}

/**
 * `<data root>/toolu/opencode-status.json` for the binding's project. In bash,
 * `shell.env` sets `TOOLU_CONFIG_DIR` to that data root, where the reader looks.
 */
export function statusRecordPath(binding: Pick<HostBinding, "projectRoot" | "env">): string {
  const dataRoot = opencodeDataRoot({ projectRoot: binding.projectRoot, env: binding.env });
  return join(opencodeRegistryRoot(dataRoot), STATUS_FILE);
}

/** Atomic: a unique temp file renamed over the record. Returns why it failed, if it did. */
export function writeStatusRecord(path: string, record: OpencodeStatusRecord): string | undefined {
  const tmp = `${path}.${randomUUID()}.tmp`;
  try {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(tmp, `${JSON.stringify(record, null, 2)}\n`, { flag: "wx" });
    renameSync(tmp, path);
    return undefined;
  } catch (error) {
    rmSync(tmp, { force: true });
    return `cannot write ${path}: ${error instanceof Error ? error.message : String(error)}`;
  }
}

/** The structured log entry's flat fields; `record` only when the file was written. */
export function statusLogExtra(
  record: OpencodeStatusRecord,
  written: string | undefined,
): LogExtra {
  const extra: LogExtra = {
    status: record.status,
    plugins: record.plugins.map((plugin) => plugin.name).join(","),
    artifacts: record.plugins.reduce((sum, plugin) => sum + plugin.artifacts, 0),
  };
  if (written !== undefined) extra.record = written;
  if (record.selection !== undefined) extra.selection = record.selection;
  if (record.reason !== undefined) extra.reason = record.reason;
  return extra;
}
