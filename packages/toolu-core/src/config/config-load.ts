/**
 * Config loader (#253): port of `toolu_load_config` / `toolu_config_exists` in
 * `plugins/toolu/hooks/lib/config.sh`. Reads `<config root>/toolu.config.json`
 * and `<project>/<dirname>/toolu.config.json`, deep-merges them like jq
 * `$u * $p` (project wins), and checks each file's envelope.
 *
 * Malformed JSON is ignored with a warning, as in bash. An envelope toolu cannot
 * understand (unknown top-level key, `version` other than 1, a non-object) marks
 * the config invalid: `data` is empty and the gate layer fails closed.
 */
import { readFileSync } from "node:fs";
import type { HostEnv, HostName } from "../host/host-name.ts";
import { configFiles, isFile } from "./config-files.ts";
import { TooluConfigSchema } from "./config-schema.ts";

export { configExists, isFile } from "./config-files.ts";

export type Warn = (message: string) => void;
export type JsonObject = { [key: string]: unknown };
export type ConfigOptions = { env?: HostEnv; host?: HostName; cwd?: string; warn?: Warn };

export type LoadedConfig = {
  /** Deep-merged user * project config; `{}` when `invalid` is set. */
  data: JsonObject;
  /** Why a file's envelope was rejected; set means fail closed. */
  invalid: string | undefined;
  files: { user: string; project: string | undefined };
  /** The host the files were resolved for; gate modes degrade `ask` for it. */
  host: HostName;
  warn: Warn;
};

const KNOWN_KEYS: ReadonlySet<string> = new Set(Object.keys(TooluConfigSchema.shape));

export function isJsonObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stderrWarn(message: string): void {
  process.stderr.write(`toolu-config: ${message}\n`);
}

/** jq `$u * $p` for two objects: nested objects merge, anything else on the project side replaces. */
function mergeObjects(user: JsonObject, project: JsonObject): JsonObject {
  const merged = new Map<string, unknown>(Object.entries(user));
  for (const [key, value] of Object.entries(project)) {
    merged.set(key, Object.hasOwn(user, key) ? mergeConfig(user[key], value) : value);
  }
  // fromEntries defines own properties, so a "__proto__" key stays data.
  return Object.fromEntries(merged);
}

/** jq `$u * $p`: objects merge recursively; anything else on the project side replaces. */
export function mergeConfig(user: unknown, project: unknown): unknown {
  return isJsonObject(user) && isJsonObject(project) ? mergeObjects(user, project) : project;
}

type FileRead = { kind: "absent" } | { kind: "malformed" } | { kind: "json"; value: unknown };

/** `[ -f ]` then `jq -e .`: null and false count as malformed, as `-e` rejects them. */
function readConfigFile(path: string): FileRead {
  if (!isFile(path)) {
    return { kind: "absent" };
  }
  try {
    const value: unknown = JSON.parse(readFileSync(path, "utf8"));
    return value === null || value === false ? { kind: "malformed" } : { kind: "json", value };
  } catch {
    return { kind: "malformed" };
  }
}

function envelopeError(value: unknown): string | undefined {
  if (!isJsonObject(value)) {
    return "top level is not a JSON object";
  }
  const unknown = Object.keys(value).filter((key) => !KNOWN_KEYS.has(key));
  if (unknown.length > 0) {
    const names = unknown.map((key) => `'${key}'`).join(", ");
    return `unknown top-level key${unknown.length === 1 ? "" : "s"} ${names}`;
  }
  if (value.version !== undefined && value.version !== 1) {
    return `unsupported version ${JSON.stringify(value.version)} (supported: 1)`;
  }
  return undefined;
}

type Layer = { value: JsonObject; invalid?: string };

function readLayer(path: string | undefined, warn: Warn): Layer {
  const read = path === undefined ? { kind: "absent" as const } : readConfigFile(path);
  if (read.kind === "absent") {
    return { value: {} };
  }
  if (read.kind === "malformed") {
    warn(`malformed JSON in ${path ?? ""}; ignoring`);
    return { value: {} };
  }
  const error = envelopeError(read.value);
  if (error !== undefined || !isJsonObject(read.value)) {
    const invalid = `${path ?? ""}: ${error ?? "top level is not a JSON object"}`;
    warn(`${invalid}; failing closed (every gate blocks)`);
    return { value: {}, invalid };
  }
  return { value: read.value };
}

/** Load and merge the user and project config for the resolved host. */
export function loadConfig(options: ConfigOptions = {}): LoadedConfig {
  const warn = options.warn ?? stderrWarn;
  const { files, host } = configFiles(options);
  const user = readLayer(files.user, warn);
  const project = readLayer(files.project, warn);
  const invalid = user.invalid ?? project.invalid;
  const data = invalid === undefined ? mergeObjects(user.value, project.value) : {};
  return { data, invalid, files, host, warn };
}
