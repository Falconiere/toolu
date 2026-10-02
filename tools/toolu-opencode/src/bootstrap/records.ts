/**
 * What one startup entry reported (#342), read strictly and then checked
 * against the disk: a registry module must be this plugin's, at its registry
 * path, byte-equal to the bundle in the selected plugin; a helper must be the
 * symlink to that bundle. Only what this run reported counts, so a stale file
 * or another plugin's module can never stand in for a missing contribution.
 */
import { lstatSync, readFileSync, readlinkSync, realpathSync, statSync } from "node:fs";
import { join, resolve, sep } from "node:path";
import { registryEventDir, registryFileName } from "@toolu/core/registry";
import type { StartupRecord } from "@toolu/core/startup";
import { z } from "zod";
import type { PluginManifest } from "../inventory/types.ts";

/** A report from a handful of modules and helpers is a few KB; anything this size is not one. */
const MAX_REPORT_BYTES = 1_000_000;

const RecordSchema: z.ZodType<StartupRecord> = z.discriminatedUnion("kind", [
  z.strictObject({
    kind: z.literal("registry"),
    spec: z.string().min(1),
    name: z.string().min(1),
    event: z.enum(["tool/pre", "tool/post"]),
    source: z.string().min(1),
    target: z.string().min(1),
    status: z.enum(["written", "unchanged", "failed"]),
    error: z.string().optional(),
  }),
  z.strictObject({
    kind: z.literal("helper"),
    plugin: z.string().min(1),
    source: z.string().min(1),
    path: z.string().min(1).optional(),
    status: z.enum(["published", "kept-user-file", "link-failed", "unwritable", "source-missing"]),
  }),
  z.strictObject({ kind: z.literal("error"), origin: z.string().min(1), message: z.string() }),
]);

export type ReportRead = { ok: true; records: StartupRecord[] } | { ok: false; reason: string };

export function readStartupReport(path: string): ReportRead {
  try {
    if (statSync(path).size > MAX_REPORT_BYTES) {
      return { ok: false, reason: `invalid startup report: larger than ${MAX_REPORT_BYTES} bytes` };
    }
    const records: StartupRecord[] = [];
    for (const line of readFileSync(path, "utf8").split("\n")) {
      if (line === "") continue;
      const parsed = RecordSchema.safeParse(JSON.parse(line));
      if (!parsed.success) {
        return { ok: false, reason: `invalid startup report: ${z.prettifyError(parsed.error)}` };
      }
      records.push(parsed.data);
    }
    return { ok: true, records };
  } catch (error) {
    return { ok: false, reason: `invalid startup report: ${String(error)}` };
  }
}

/** A helper this run published and toolu therefore owns. */
export type OwnedHelper = { path: string; source: string };

export type Verified = {
  artifacts: string[];
  helpers: OwnedHelper[];
  diagnostics: string[];
  failures: string[];
};

type Scope = { plugin: PluginManifest; dataRoot: string; pluginReal: string };

function inside(path: string, root: string): boolean {
  return path.startsWith(root + sep);
}

/** `source` resolves inside the selected plugin; a missing file is not inside anything. */
function fromPlugin(source: string, scope: Scope): boolean {
  try {
    return inside(realpathSync(source), scope.pluginReal);
  } catch {
    return false;
  }
}

function verifyRegistry(
  record: Extract<StartupRecord, { kind: "registry" }>,
  scope: Scope,
  out: Verified,
): void {
  const { plugin, dataRoot } = scope;
  if (record.status === "failed") {
    out.failures.push(`${record.target}: ${record.error ?? "registration failed"}`);
    return;
  }
  const options = { env: { TOOLU_CONFIG_DIR: dataRoot }, host: "opencode" as const };
  const expected = join(
    registryEventDir(record.event, options),
    registryFileName(plugin.spec, record.name),
  );
  if (
    record.spec !== plugin.spec ||
    resolve(record.target) !== resolve(expected) ||
    !fromPlugin(record.source, scope)
  ) {
    out.failures.push(`${record.target}: contribution outside ${plugin.name}`);
    return;
  }
  try {
    if (!readFileSync(record.target).equals(readFileSync(record.source))) {
      out.failures.push(`${record.target} is not the current bundle`);
      return;
    }
  } catch (error) {
    out.failures.push(`${record.target}: ${String(error)}`);
    return;
  }
  out.artifacts.push(record.target);
}

function isLinkTo(path: string, source: string): boolean {
  try {
    return lstatSync(path).isSymbolicLink() && readlinkSync(path) === source;
  } catch {
    return false;
  }
}

function verifyHelper(
  record: Extract<StartupRecord, { kind: "helper" }>,
  scope: Scope,
  out: Verified,
): void {
  const { plugin, dataRoot } = scope;
  const where = record.path ?? record.source;
  if (record.plugin !== plugin.name) {
    out.failures.push(`${where}: contribution outside ${plugin.name}`);
  } else if (record.status === "kept-user-file") {
    out.diagnostics.push(`${plugin.name}: kept user file ${where}`);
  } else if (record.status !== "published" || record.path === undefined) {
    out.failures.push(`helper ${where}: ${record.status}`);
  } else if (
    !inside(resolve(record.path), resolve(dataRoot)) ||
    !fromPlugin(record.source, scope)
  ) {
    out.failures.push(`${where}: contribution outside ${plugin.name}`);
  } else if (!isLinkTo(record.path, record.source)) {
    out.failures.push(`helper ${where} is not a link to ${record.source}`);
  } else {
    out.artifacts.push(record.path);
    out.helpers.push({ path: record.path, source: record.source });
  }
}

export function verifyRecords(
  records: readonly StartupRecord[],
  plugin: PluginManifest,
  dataRoot: string,
): Verified {
  const out: Verified = { artifacts: [], helpers: [], diagnostics: [], failures: [] };
  let pluginReal: string;
  try {
    pluginReal = realpathSync(plugin.pluginDir);
  } catch (error) {
    out.failures.push(`plugin directory ${plugin.pluginDir} is gone: ${String(error)}`);
    return out;
  }
  const scope: Scope = { plugin, dataRoot, pluginReal };
  for (const record of records) {
    if (record.kind === "registry") verifyRegistry(record, scope, out);
    else if (record.kind === "helper") verifyHelper(record, scope, out);
    else out.failures.push(`${record.origin}: ${record.message}`);
  }
  return out;
}
