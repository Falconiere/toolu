/**
 * Codex plugin snapshot (#252). Written once at Codex SessionStart from
 * `codex plugin list --json` so hot-path readers never spawn the CLI, and read
 * as a tri-state: a stale or failed snapshot is `unknown`, never "absent".
 * Output is byte-identical to bash `toolu_snapshot_codex_plugins`.
 */
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { z } from "zod";
import { envValue, type HostEnv } from "./host-name.ts";
import { configRoot, resolveHost, type HostOptions } from "./host-roots.ts";

export type CodexPluginSnapshot = {
  version: 1;
  status: "ready" | "indeterminate";
  plugins: string[];
};

export type SnapshotResult = { path: string; snapshot: CodexPluginSnapshot; written: boolean };

const INDETERMINATE: CodexPluginSnapshot = { version: 1, status: "indeterminate", plugins: [] };

const ListSchema = z.looseObject({ installed: z.array(z.unknown()) });
const EntrySchema = z.looseObject({
  pluginId: z.unknown(),
  name: z.unknown(),
  marketplaceName: z.unknown(),
});
const SnapshotFileSchema = z.looseObject({
  version: z.literal(1),
  status: z.string(),
  plugins: z.array(z.unknown()),
});

/** `TOOLU_CODEX_PLUGIN_SNAPSHOT`, else `<config root>/toolu/codex-plugins.json`. */
export function codexPluginSnapshotPath(options: HostOptions = {}): string {
  const { env } = resolveHost(options);
  return (
    envValue(env, "TOOLU_CODEX_PLUGIN_SNAPSHOT") ??
    join(configRoot(options), "toolu", "codex-plugins.json")
  );
}

function childEnv(env: HostEnv): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value;
  }
  return out;
}

function readCodexList(env: HostEnv): unknown {
  const res = spawnSync("codex", ["plugin", "list", "--json"], {
    env: childEnv(env),
    encoding: "utf8",
  });
  if (res.error !== undefined || res.status !== 0) {
    return undefined;
  }
  try {
    const parsed: unknown = JSON.parse(res.stdout);
    return parsed;
  } catch {
    return undefined;
  }
}

/** A flag is on when the key is absent or exactly `true` (jq `has | not or == true`). */
function flagOn(entry: Record<string, unknown>, key: string): boolean {
  return !Object.hasOwn(entry, key) || entry[key] === true;
}

/** `.pluginId // "<name>@<marketplaceName>"`, kept only as a non-empty string. */
function entryId(entry: z.infer<typeof EntrySchema>): string | undefined {
  const { pluginId, name, marketplaceName } = entry;
  const id =
    pluginId !== null && pluginId !== false && pluginId !== undefined
      ? pluginId
      : typeof name === "string" && typeof marketplaceName === "string"
        ? `${name}@${marketplaceName}`
        : undefined;
  return typeof id === "string" && id.length > 0 ? id : undefined;
}

/** Canonical snapshot for parsed CLI output; any shape jq would reject is indeterminate. */
function canonicalSnapshot(raw: unknown): CodexPluginSnapshot {
  const list = ListSchema.safeParse(raw);
  if (!list.success) {
    return INDETERMINATE;
  }
  const ids = new Set<string>();
  for (const item of list.data.installed) {
    const entry = EntrySchema.safeParse(item);
    if (!entry.success || Array.isArray(item)) {
      return INDETERMINATE;
    }
    const id =
      flagOn(entry.data, "installed") && flagOn(entry.data, "enabled")
        ? entryId(entry.data)
        : undefined;
    if (id !== undefined) ids.add(id);
  }
  return { version: 1, status: "ready", plugins: [...ids].toSorted() };
}

/** Atomic write; `false` when the location is unwritable (readers then see `unknown`). */
function writeAtomically(path: string, body: string): boolean {
  const tmp = `${path}.tmp.${process.pid}`;
  let created = false;
  try {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(tmp, body);
    created = true;
    renameSync(tmp, path);
    return true;
  } catch {
    if (created) rmSync(tmp, { force: true });
    return false;
  }
}

/** Refresh the snapshot on Codex; `undefined` (nothing written) on any other host. */
export function snapshotCodexPlugins(options: HostOptions = {}): SnapshotResult | undefined {
  const { env, host } = resolveHost(options);
  if (host !== "codex") {
    return undefined;
  }
  const path = codexPluginSnapshotPath({ env, host });
  const snapshot = canonicalSnapshot(readCodexList(env));
  const written = writeAtomically(path, `${JSON.stringify(snapshot)}\n`);
  return { path, snapshot, written };
}

function readSnapshotFile(path: string): z.infer<typeof SnapshotFileSchema> | undefined {
  try {
    const parsed = SnapshotFileSchema.safeParse(JSON.parse(readFileSync(path, "utf8")));
    return parsed.success ? parsed.data : undefined;
  } catch {
    return undefined;
  }
}

/** `installed` / `absent` from a ready snapshot; `unknown` when there is no trustworthy one. */
export function codexPluginInstalled(
  spec: string,
  options: HostOptions = {},
): "installed" | "absent" | "unknown" {
  if (spec === "") {
    return "absent";
  }
  const snapshot = readSnapshotFile(codexPluginSnapshotPath(options));
  if (snapshot?.status !== "ready") {
    return "unknown";
  }
  return snapshot.plugins.includes(spec) ? "installed" : "absent";
}
