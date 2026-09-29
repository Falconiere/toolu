/**
 * Config readers (#253): port of the `config.sh` public API over a loaded
 * config. Each mirrors its jq filter, including how a wrong-typed section or
 * value falls back, so resolved values match bash on the same files.
 */
import { isJsonObject, type JsonObject, type LoadedConfig } from "./config-load.ts";

export const MODEL_CLASSES = [
  "mechanical",
  "exploration",
  "implementation",
  "review",
  "synthesis",
  "architecture",
] as const;
export type ModelClass = (typeof MODEL_CLASSES)[number];

export const MODEL_ALIASES = ["haiku", "sonnet", "opus", "fable", "inherit"] as const;
export type ModelAlias = (typeof MODEL_ALIASES)[number];

export const CODEX_REASONING_EFFORTS = ["low", "medium", "high", "xhigh", "max", "ultra"] as const;
export type ReasoningEffort = (typeof CODEX_REASONING_EFFORTS)[number];

export type CodexModel = { model: string; reasoningEffort: ReasoningEffort };

const MODEL_DEFAULTS: Readonly<Record<ModelClass, ModelAlias>> = {
  mechanical: "haiku",
  exploration: "sonnet",
  implementation: "sonnet",
  review: "sonnet",
  synthesis: "opus",
  architecture: "opus",
};

const CODEX_DEFAULTS: Readonly<Record<ModelClass, CodexModel>> = {
  mechanical: { model: "gpt-5.6-luna", reasoningEffort: "medium" },
  exploration: { model: "gpt-5.6-terra", reasoningEffort: "medium" },
  implementation: { model: "gpt-5.6-terra", reasoningEffort: "medium" },
  review: { model: "gpt-5.6-terra", reasoningEffort: "high" },
  synthesis: { model: "gpt-5.6-sol", reasoningEffort: "high" },
  architecture: { model: "gpt-5.6-sol", reasoningEffort: "high" },
};

/** `(.[$c]? // {})`: the section when it is an object. */
export function section(config: LoadedConfig, key: string): JsonObject | undefined {
  const value = config.data[key];
  return isJsonObject(value) ? value : undefined;
}

function member(config: LoadedConfig, category: string, name: string): unknown {
  return section(config, category)?.[name];
}

/** `toolu_enabled`: default on; off only for `false` or the string `"false"`. */
export function enabled(config: LoadedConfig, category: string, name: string): boolean {
  const value = member(config, category, name);
  return value !== false && value !== "false";
}

/** `toolu_flag_true`: on only for the JSON boolean `true`. */
export function flagTrue(config: LoadedConfig, category: string, name: string): boolean {
  return member(config, category, name) === true;
}

/** `toolu_flag_false`: an explicit opt-out, the JSON boolean `false` only. */
export function flagFalse(config: LoadedConfig, category: string, name: string): boolean {
  return member(config, category, name) === false;
}

/** `toolu_enabled_explicit`: default off; on for `true` or the string `"true"`. */
export function enabledExplicit(config: LoadedConfig, category: string, name: string): boolean {
  const value = member(config, category, name);
  return value === true || value === "true";
}

type PathRead = { found: false } | { found: true; value: unknown };

/**
 * jq `getpath`: a missing key or null ends the walk at null; indexing a
 * scalar or array is a jq error, which bash turns into the default too.
 */
function readPath(data: JsonObject, path: string): PathRead {
  let current: unknown = data;
  for (const key of path.split(".")) {
    if (!isJsonObject(current)) {
      return { found: false };
    }
    current = current[key];
  }
  return current === undefined || current === null
    ? { found: false }
    : { found: true, value: current };
}

function isOneOf<T extends string>(value: string, allowed: readonly T[]): value is T {
  return allowed.some((item) => item === value);
}

/**
 * `toolu_string PATH DEFAULT ALLOWED...`: the string at dotted `path` when it
 * is one of `allowed`. Absent falls back silently; present but unqualified
 * warns, then falls back.
 */
export function configString<T extends string>(
  config: LoadedConfig,
  path: string,
  fallback: T,
  allowed: readonly T[],
): T {
  const read = readPath(config.data, path);
  if (!read.found) {
    return fallback;
  }
  if (typeof read.value !== "string") {
    config.warn(`${path}: value is not a string; using ${fallback}`);
    return fallback;
  }
  if (isOneOf(read.value, allowed)) {
    return read.value;
  }
  config.warn(
    `${path}: '${read.value}' is not an allowed value (${allowed.join(" ")}); using ${fallback}`,
  );
  return fallback;
}

function requireClass(cls: string): ModelClass {
  if (!isOneOf(cls, MODEL_CLASSES)) {
    throw new TypeError(`unknown model class '${cls}' (known: ${MODEL_CLASSES.join(" ")})`);
  }
  return cls;
}

/** jq `tostring`: strings as-is, everything else as compact JSON. */
function jqToString(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

/** `toolu_model CLASS`: `.models.<class>` when it is an alias, else the default. */
export function model(config: LoadedConfig, cls: string): ModelAlias {
  const known = requireClass(cls);
  const fallback = MODEL_DEFAULTS[known];
  const value = section(config, "models")?.[known];
  // jq `// ""` also swallows false, and an empty string means unset.
  if (value === undefined || value === null || value === false || value === "") {
    return fallback;
  }
  const text = jqToString(value);
  if (isOneOf(text, MODEL_ALIASES)) {
    return text;
  }
  config.warn(
    `models.${known}: '${text}' is not a model alias (${MODEL_ALIASES.join(" ")}); using ${fallback}`,
  );
  return fallback;
}

/** `.models.codex.<class>.<field>` through objects only; null when unreachable. */
function codexField(config: LoadedConfig, cls: ModelClass, field: string): unknown {
  const codex = section(config, "models")?.codex;
  const entry = isJsonObject(codex) ? codex[cls] : undefined;
  const value = isJsonObject(entry) ? entry[field] : undefined;
  return value === undefined || value === false ? null : value;
}

/** `toolu_codex_model CLASS`: model slug and reasoning effort, each falling back alone. */
export function codexModel(config: LoadedConfig, cls: string): CodexModel {
  const known = requireClass(cls);
  const fallback = CODEX_DEFAULTS[known];
  const prefix = `models.codex.${known}`;

  let slug = fallback.model;
  const rawModel = codexField(config, known, "model");
  if (typeof rawModel === "string" && rawModel !== "") {
    slug = rawModel;
  } else if (rawModel !== null) {
    config.warn(`${prefix}.model: value is not a non-empty string; using ${fallback.model}`);
  }

  let effort = fallback.reasoningEffort;
  const rawEffort = codexField(config, known, "reasoningEffort");
  if (typeof rawEffort === "string") {
    if (isOneOf(rawEffort, CODEX_REASONING_EFFORTS)) {
      effort = rawEffort;
    } else {
      config.warn(
        `${prefix}.reasoningEffort: '${rawEffort}' is not supported (${CODEX_REASONING_EFFORTS.join(" ")}); using ${fallback.reasoningEffort}`,
      );
    }
  } else if (rawEffort !== null) {
    config.warn(
      `${prefix}.reasoningEffort: value is not a string; using ${fallback.reasoningEffort}`,
    );
  }
  return { model: slug, reasoningEffort: effort };
}
