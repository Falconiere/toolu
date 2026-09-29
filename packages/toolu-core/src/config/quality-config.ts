/**
 * Quality thresholds (#253): port of `plugins/toolu/hooks/lib/quality-config.sh`.
 * Each limit resolves override (`lang.<lang>.<key>`) → native linter config (TS
 * `maxFileLines` only: the active linter's `max-lines`) → built-in default.
 * Every layer that cannot produce a positive number falls through; nothing
 * here throws or warns.
 */
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import type { HostEnv } from "../host/host-name.ts";
import { gitToplevel } from "../host/host-roots.ts";
import { isFile, isJsonObject, type LoadedConfig } from "./config-load.ts";
import { section } from "./config-read.ts";

export const QUALITY_DEFAULTS = {
  ts: { maxFileLines: 300, maxFnLines: 60 },
  rust: { maxFileLines: 500, maxFnLines: 50, maxImplLines: 200 },
  python: { maxFileLines: 400, maxFnLines: 50 },
} as const;

export type QualityLang = keyof typeof QUALITY_DEFAULTS;
export type QualityKey<L extends QualityLang> = keyof (typeof QUALITY_DEFAULTS)[L] & string;
export type ThresholdSource = "override" | "native" | "default";
export type QualityOptions = { env?: HostEnv; cwd?: string; root?: string };

/** jq `tonumber?`: surrounding whitespace, a sign, `.5`, `5.` and exponents parse. */
const JQ_NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

/** jq `if string then tonumber? else . end | if number and > 0 then floor`. */
function positiveFloor(value: unknown): number | undefined {
  let number = value;
  if (typeof value === "string") {
    const trimmed = value.trim();
    number = JQ_NUMBER.test(trimmed) ? Number(trimmed) : undefined;
  }
  return typeof number === "number" && number > 0 ? Math.floor(number) : undefined;
}

function langMember(config: LoadedConfig, lang: string, key: string): unknown {
  const entry = section(config, "lang")?.[lang];
  return isJsonObject(entry) ? entry[key] : undefined;
}

/**
 * The `max-lines` rule of a parsed eslint/oxlint config: `N`, `["error", N]`,
 * `["error", {"max": N}]`; severity `"off"`/`0` and non-positive values yield
 * `undefined`.
 */
export function nativeMaxLines(linterConfig: unknown): number | undefined {
  const rules = isJsonObject(linterConfig) ? linterConfig.rules : undefined;
  const rule = isJsonObject(rules) ? rules["max-lines"] : undefined;
  if (Array.isArray(rule)) {
    const items: readonly unknown[] = rule;
    const severity = items[0];
    const option = items[1];
    if (severity === "off" || severity === 0) {
      return undefined;
    }
    return positiveFloor(isJsonObject(option) ? option.max : option);
  }
  return typeof rule === "number" || typeof rule === "string" ? positiveFloor(rule) : undefined;
}

/** `detect_ts_linter`'s machine-readable config: biome > oxc > eslint. */
function activeLinterConfig(root: string): string | undefined {
  if (isFile(join(root, "biome.json")) || isFile(join(root, "biome.jsonc"))) {
    return undefined;
  }
  if (isFile(join(root, ".oxlintrc.json"))) {
    return join(root, ".oxlintrc.json");
  }
  let names: string[];
  try {
    names = readdirSync(root);
  } catch {
    return undefined;
  }
  const eslint = names.some(
    (name) => name.startsWith(".eslintrc") || name.startsWith("eslint.config."),
  );
  return eslint ? join(root, ".eslintrc.json") : undefined;
}

function nativeTsMaxLines(options: QualityOptions): number | undefined {
  const root = options.root ?? gitToplevel(options.env ?? process.env, options.cwd);
  const file = root === undefined ? undefined : activeLinterConfig(root);
  if (file === undefined || !isFile(file)) {
    return undefined;
  }
  try {
    return nativeMaxLines(JSON.parse(readFileSync(file, "utf8")));
  } catch {
    return undefined;
  }
}

/** `quality_threshold`: always a number (bash floors, so a 0.5 override yields 0). */
export function qualityThreshold<L extends QualityLang>(
  config: LoadedConfig,
  lang: L,
  key: QualityKey<L>,
  options: QualityOptions = {},
): number {
  const override = positiveFloor(langMember(config, lang, key));
  if (override !== undefined) {
    return override;
  }
  if (lang === "ts" && key === "maxFileLines") {
    const native = nativeTsMaxLines(options);
    if (native !== undefined) {
      return native;
    }
  }
  const defaults: Readonly<Record<string, number>> = QUALITY_DEFAULTS[lang];
  return defaults[key] ?? 0;
}

/** `ts_max_file_lines_resolved`: the TS file limit and the layer it came from. */
export function tsMaxFileLinesResolved(
  config: LoadedConfig,
  options: QualityOptions = {},
): { value: number; source: ThresholdSource } {
  const override = positiveFloor(langMember(config, "ts", "maxFileLines"));
  if (override !== undefined) {
    return { value: override, source: "override" };
  }
  const native = nativeTsMaxLines(options);
  return native === undefined
    ? { value: QUALITY_DEFAULTS.ts.maxFileLines, source: "default" }
    : { value: native, source: "native" };
}

/** `quality_flag`: a literal JSON boolean at `lang.<lang>.<key>`, else `fallback`. */
export function qualityFlag(
  config: LoadedConfig,
  lang: QualityLang,
  key: string,
  fallback: boolean,
): boolean {
  const value = langMember(config, lang, key);
  return typeof value === "boolean" ? value : fallback;
}
