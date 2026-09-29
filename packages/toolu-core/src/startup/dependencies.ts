/**
 * Plugin dependency checks (#269). Claude Code resolves `plugin.json`
 * dependencies itself; Codex has no such manifest field, so a dependent
 * plugin asks `codex plugin list --json` at SessionStart and warns with the
 * exact install command. Port of `check-toolu.sh` and `check-deps.sh`,
 * including their jq semantics.
 *
 * The live CLI is read, not the Codex plugin snapshot: toolu's own
 * SessionStart writes that snapshot, so it is absent exactly when the core
 * is missing.
 */
import { spawnSync } from "node:child_process";
import { detectHost } from "../host/host-detect.ts";
import { childEnv, envValue, type HostEnv } from "../host/host-name.ts";
import { pluginInstallCommand } from "../host/host-roots.ts";
import { renderHookOutput, sessionContext } from "./context.ts";

export const CORE_PLUGIN = "toolu@toolu";

type Listing = { ok: true; value: unknown } | { ok: false };

/** `codex plugin list --json`; `undefined` when the CLI is absent or exits non-zero. */
function codexListing(env: HostEnv): Listing | undefined {
  const res = spawnSync("codex", ["plugin", "list", "--json"], {
    env: childEnv(env),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
  if (res.error !== undefined || res.status !== 0) return undefined;
  try {
    const value: unknown = JSON.parse(res.stdout);
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** `(has(key) | not) or (.[key] == true)`. */
function flagOn(entry: Record<string, unknown>, key: string): boolean {
  return !Object.hasOwn(entry, key) || entry[key] === true;
}

/** `.installed[]?`: an array's items or an object's values; nothing for any other shape. */
function entries(listing: unknown): unknown[] {
  if (!isRecord(listing)) return [];
  const installed = listing["installed"];
  if (Array.isArray(installed)) return installed;
  return isRecord(installed) ? Object.values(installed) : [];
}

/**
 * jq `any(.installed[]?; .pluginId == $id and <installed> and <enabled>)` under
 * `-e`: true only on a match. An entry jq cannot index (a string, number,
 * boolean or array) raises, and a raised check counts as not installed unless
 * an earlier entry already matched.
 */
function isInstalled(listing: unknown, id: string): boolean {
  for (const entry of entries(listing)) {
    if (entry === null) continue;
    if (!isRecord(entry)) return false;
    if (entry["pluginId"] === id && flagOn(entry, "installed") && flagOn(entry, "enabled")) {
      return true;
    }
  }
  return false;
}

/**
 * The `required` plugin ids Codex does not list as installed and enabled, in
 * order. `undefined` means "do not check": not the Codex host, `PLUGIN_ROOT`
 * unset, the CLI absent, or `codex plugin list` failing. Output that is not
 * JSON counts every id as missing, as jq did.
 */
export function codexMissingPlugins(
  required: readonly string[],
  env: HostEnv = process.env,
): string[] | undefined {
  if (detectHost({ env }) !== "codex" || envValue(env, "PLUGIN_ROOT") === undefined) {
    return undefined;
  }
  const listing = codexListing(env);
  if (listing === undefined) return undefined;
  return required.filter((id) => !listing.ok || !isInstalled(listing.value, id));
}

function installCommand(spec: string): string {
  return pluginInstallCommand(spec, { host: "codex" }) ?? spec;
}

/** The core-dependency warning of `check-toolu.sh`. */
export function requiresCoreWarning(): string {
  return `WARN: this plugin requires the toolu core. Install it first with: ${installCommand(CORE_PLUGIN)}`;
}

/** The multi-plugin warning of `check-deps.sh`: each missing id with its install command. */
export function requiresPluginsWarning(missing: readonly string[]): string {
  const parts = missing.map((id) => ` ${id} (install with: ${installCommand(id)})`);
  return `WARN: this plugin requires${parts.join("")}`;
}

/**
 * The whole dependency hook: the pretty-printed SessionStart context naming
 * what is missing, or `undefined` when there is nothing to say. `core` uses
 * the single-core wording; `each` lists every missing plugin.
 */
export function codexDependencyNotice(
  required: readonly string[],
  style: "core" | "each",
  env: HostEnv = process.env,
): string | undefined {
  const missing = codexMissingPlugins(required, env);
  if (missing === undefined || missing.length === 0) return undefined;
  const text = style === "core" ? requiresCoreWarning() : requiresPluginsWarning(missing);
  return renderHookOutput(sessionContext("SessionStart", text), true);
}
