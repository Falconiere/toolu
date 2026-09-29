/**
 * Installed-plugin gating for registry modules (#257), a port of
 * `detect_plugin_installed` and `toolu_plugin_active`. A module runs unless its
 * plugin is definitively absent: an unreadable install record fails open, so a
 * moved or malformed file never silently switches enforcement off.
 */
import { readFileSync, statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import { envValue, type HostEnv } from "../host/host-name.ts";
import { resolveHost, type HostOptions } from "../host/host-roots.ts";
import { codexPluginInstalled } from "../host/host-snapshot.ts";

export type PluginPresence = "installed" | "absent" | "unknown";

/** Claude Code's install record, resolved like `detect.sh` (and `registry.sh`'s root). */
function installedPluginsPath(env: HostEnv): string {
  const root =
    envValue(env, "TOOLU_CONFIG_DIR") ??
    envValue(env, "CLAUDE_CONFIG_DIR") ??
    join(envValue(env, "HOME") ?? homedir(), ".claude");
  return (
    envValue(env, "CLAUDE_PLUGINS_REGISTRY") ?? join(root, "plugins", "installed_plugins.json")
  );
}

function readInstalledPlugins(path: string): unknown {
  try {
    if (!statSync(path).isFile()) return undefined;
    const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
    return parsed;
  } catch {
    return undefined;
  }
}

function claudePresence(spec: string, env: HostEnv): PluginPresence {
  const doc = readInstalledPlugins(installedPluginsPath(env));
  if (!isJsonObject(doc) || !isJsonObject(doc.plugins)) return "unknown";
  return Object.hasOwn(doc.plugins, spec) ? "installed" : "absent";
}

/**
 * Whether plugin `spec` (`name@marketplace`) is installed. Claude reads
 * `installed_plugins.json`, Codex its SessionStart snapshot. Cursor, Hermes
 * and OpenCode keep no install record toolu can read, so they are `unknown`.
 */
export function pluginPresence(spec: string, options: HostOptions = {}): PluginPresence {
  if (spec === "") return "absent";
  const o = resolveHost(options);
  if (o.host === "codex") return codexPluginInstalled(spec, o);
  if (o.host !== "claude") return "unknown";
  return claudePresence(spec, o.env);
}

/** Registry modules of `spec` run unless the plugin is definitively absent. */
export function pluginActive(spec: string, options: HostOptions = {}): boolean {
  return pluginPresence(spec, options) !== "absent";
}
