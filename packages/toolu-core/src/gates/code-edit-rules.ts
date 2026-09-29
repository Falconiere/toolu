/**
 * code-edit-rules (#260), the native port of `pre-tools/modules/code-edit-rules.sh`.
 * Before an Edit/Write/MultiEdit, the first rule of `settings/code-edit-rules.json`
 * whose `match` glob fits the repo-relative path adds its `docs` (and its
 * `extra_docs` when a `when_path_matches` glob also fits) as context. The file
 * is read the way the bash `jq` calls read it: field by field with defaults, so
 * an extra key or a missing `docs` never switches off the other rules.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { isFile, isJsonObject } from "../config/config-load.ts";
import { SETTINGS_FILES } from "../config/settings.ts";
import type { Decision } from "../decision/decision.ts";
import { gitToplevel } from "../host/host-roots.ts";
import type { RegistryContext, RegistryHookEvent } from "../registry/registry-types.ts";
import { bashPatternMatch } from "./bash-pattern.ts";
import { repoRelative } from "./gate-paths.ts";
import {
  ALLOW,
  gateSettingsDir,
  inputString,
  type GateModule,
  type GateModuleOptions,
} from "./gate-module.ts";

const EDIT_TOOLS = new Set(["Edit", "Write", "MultiEdit"]);

/** `jq -r` of a scalar: strings as-is, numbers and booleans as JSON text; else undefined. */
function scalarText(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return JSON.stringify(value);
  return undefined;
}

/** `.<key> // [] | join(" + ")`: "" where jq would error (a non-array, an object member). */
function joined(value: unknown): string {
  if (!Array.isArray(value)) return "";
  const parts = value.map((item: unknown) => (item === null ? "" : scalarText(item)));
  return parts.every((part) => part !== undefined) ? parts.join(" + ") : "";
}

/** `.when_path_matches[$j]` as `jq -r` prints each member; a non-array has none. */
function patterns(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item: unknown) => {
    const text = item === null ? "null" : scalarText(item);
    return text === undefined ? [] : [text];
  });
}

function readRules(path: string): unknown[] {
  try {
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    const rules = isJsonObject(parsed) ? parsed.rules : undefined;
    return Array.isArray(rules) ? rules : [];
  } catch {
    return [];
  }
}

/** The docs of the first rule whose `match` fits `rel`, joined with ` + `. */
function docsFor(rules: readonly unknown[], rel: string): string {
  for (const rule of rules) {
    if (!isJsonObject(rule)) continue;
    const match = rule.match === false ? undefined : scalarText(rule.match);
    if (match === undefined || match === "" || !bashPatternMatch(match, rel)) continue;
    const docs = [joined(rule.docs)];
    if (patterns(rule.when_path_matches).some((cond) => bashPatternMatch(cond, rel))) {
      docs.push(joined(rule.extra_docs));
    }
    return docs.filter((d) => d !== "" && d !== "null").join(" + ");
  }
  return "";
}

function decide(
  event: RegistryHookEvent,
  ctx: RegistryContext,
  options: GateModuleOptions,
): Decision {
  if (event.type !== "tool/pre" || !EDIT_TOOLS.has(event.toolName)) return ALLOW;
  const dir = gateSettingsDir(ctx, options);
  const file = dir === undefined ? undefined : join(dir, SETTINGS_FILES.codeEditRules);
  if (file === undefined || !isFile(file)) return ALLOW;
  const path = inputString(event, "file_path");
  if (path === "") return ALLOW;
  const root = path.startsWith("/") ? gitToplevel(ctx.env, ctx.cwd ?? process.cwd()) : undefined;
  const rel = repoRelative(path, root);
  const docs = docsFor(readRules(file), rel);
  if (docs === "") return ALLOW;
  return { kind: "advisory", message: `File: ${rel}\nApply these rules: ${docs}` };
}

/** The code-edit-rules built-in module. */
export function codeEditRulesModule(options: GateModuleOptions = {}): GateModule {
  return {
    kind: "native",
    name: "code-edit-rules",
    run: (event, ctx) => Promise.resolve(decide(event, ctx, options)),
  };
}
