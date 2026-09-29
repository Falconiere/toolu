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
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { detectHost } from "../host/host-detect.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import { configRoot, projectConfigPath } from "../host/host-roots.ts";
import { TooluConfigSchema } from "./config-schema.ts";

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

/** jq `$u * $p`: objects merge recursively; anything else on the project side replaces. */
export function mergeConfig(user: unknown, project: unknown): unknown {
  if (!isJsonObject(user) || !isJsonObject(project)) {
    return project;
  }
  const merged = new Map<string, unknown>(Object.entries(user));
  for (const [key, value] of Object.entries(project)) {
    merged.set(key, Object.hasOwn(user, key) ? mergeConfig(user[key], value) : value);
  }
  // fromEntries defines own properties, so a "__proto__" key stays data.
  return Object.fromEntries(merged);
}

/** `[ -f PATH ]`: a regular file (or a symlink to one). */
export function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
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
  if (Object.hasOwn(value, "version") && value.version !== 1) {
    return `unsupported version ${JSON.stringify(value.version)} (supported: 1)`;
  }
  return undefined;
}

function configFiles(options: ConfigOptions): Pick<LoadedConfig, "files" | "host"> {
  const env = options.env ?? process.env;
  const host = options.host ?? detectHost({ env });
  const scoped = options.cwd === undefined ? { env, host } : { env, host, cwd: options.cwd };
  const files = {
    user: join(configRoot(scoped), "toolu.config.json"),
    project: projectConfigPath(scoped),
  };
  return { files, host };
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
  const merged = mergeConfig(user.value, project.value);
  const data = invalid === undefined && isJsonObject(merged) ? merged : {};
  return { data, invalid, files, host, warn };
}

/** Is either config file on disk? Stat only: no read, no parse. */
export function configExists(options: ConfigOptions = {}): boolean {
  const { files } = configFiles(options);
  return isFile(files.user) || (files.project !== undefined && isFile(files.project));
}
