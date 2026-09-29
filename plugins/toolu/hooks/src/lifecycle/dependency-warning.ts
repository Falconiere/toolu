/**
 * The missing-dependency warning of toolu's SessionStart (#263): each
 * `dependencies` entry of the running plugin's manifest that is definitively
 * not installed gets its host-native install command. An indeterminate
 * install record silences the whole block rather than listing every entry.
 */
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { isJsonObject } from "@toolu/core/config";
import type { HostName } from "@toolu/core/host";
import { installCommand, presence } from "./plugin-presence.ts";

type Env = Record<string, string | undefined>;

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function readJson(path: string): unknown {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return undefined;
  }
}

/** jq string interpolation: strings raw, anything else as compact JSON. */
function interpolate(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

/** One `(.dependencies // [])[]` member as the bash `jq -r` printed it. */
function specOf(entry: unknown): string | undefined {
  if (typeof entry === "string") return entry;
  if (!isJsonObject(entry) || typeof entry.name !== "string") return undefined;
  const market = entry.marketplace;
  return market === undefined || market === null || market === false
    ? entry.name
    : `${entry.name}@${interpolate(market)}`;
}

/** The spec lines `read -r` saw: empty lines skipped, embedded newlines split. */
export function dependencySpecs(manifest: unknown): string[] {
  const deps = isJsonObject(manifest) ? manifest.dependencies : undefined;
  let entries: unknown[] = [];
  if (Array.isArray(deps)) entries = deps;
  else if (isJsonObject(deps)) entries = Object.values(deps);
  return entries
    .map(specOf)
    .filter((spec) => spec !== undefined)
    .flatMap((spec) => spec.split("\n"))
    .filter((line) => line !== "");
}

/** The manifest session-start.sh read: host-native, then Claude's, then the toolu checkout's. */
function manifestPath(pluginRoot: string, projectRoot: string, host: HostName): string | undefined {
  const codex = join(pluginRoot, ".codex-plugin", "plugin.json");
  const claude = join(pluginRoot, ".claude-plugin", "plugin.json");
  if (host === "codex" && pluginRoot !== "" && isFile(codex)) {
    const doc = readJson(codex);
    const hasDeps = isJsonObject(doc) && Array.isArray(doc.dependencies);
    return hasDeps || !isFile(claude) ? codex : claude;
  }
  if (pluginRoot !== "" && isFile(claude)) return claude;
  const checkout = join(projectRoot, "plugins", "toolu", ".claude-plugin", "plugin.json");
  return isFile(checkout) ? checkout : undefined;
}

export type DependencyInput = { env: Env; host: HostName; pluginRoot: string; projectRoot: string };

/** The WARN block, or undefined when nothing is definitively missing. */
export function dependencyWarning(input: DependencyInput): string | undefined {
  const path = manifestPath(input.pluginRoot, input.projectRoot, input.host);
  if (path === undefined) return undefined;
  const missing: string[] = [];
  for (const spec of dependencySpecs(readJson(path))) {
    const state = presence(spec, input.env, input.host);
    if (state === "unknown") return undefined;
    if (state === "absent") missing.push(installCommand(spec, input.host));
  }
  if (missing.length === 0) return undefined;
  const lines = missing.map((command) => `\n  • ${command}`).join("");
  return `WARN: required plugins missing — dependent workflows will fail. Install:${lines}`;
}
