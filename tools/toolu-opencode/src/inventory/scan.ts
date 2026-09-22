/** Scan plugins root for manifests; never reads Claude install sets (#211). */
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { readPluginManifest } from "./manifest.ts";
import type { PluginInventoryEntry, PluginManifest } from "./types.ts";

export function listPluginManifests(pluginsRoot: string): PluginManifest[] | null {
  let entries: string[];
  try {
    entries = readdirSync(pluginsRoot);
  } catch {
    return null;
  }

  const manifests: PluginManifest[] = [];
  for (const entry of entries) {
    const pluginDir = join(pluginsRoot, entry);
    try {
      if (!statSync(pluginDir).isDirectory()) {
        continue;
      }
    } catch {
      continue;
    }
    const manifest = readPluginManifest(pluginDir);
    if (manifest) {
      manifests.push(manifest);
    }
  }
  return manifests;
}

export function lookupPluginInstallState(
  pluginsRoot: string,
  pluginName: string,
): PluginInventoryEntry["install"] {
  const manifests = listPluginManifests(pluginsRoot);
  if (manifests === null) {
    return "unknown";
  }
  const hit = manifests.some((m) => m.name === pluginName);
  return hit ? "installed" : "absent";
}

export function buildInventory(
  pluginsRoot: string,
  enabledNames: Set<string> | null,
): PluginInventoryEntry[] | null {
  const manifests = listPluginManifests(pluginsRoot);
  if (manifests === null) {
    return null;
  }
  return manifests.map((manifest) => {
    let enabled: PluginInventoryEntry["enabled"] = "unknown";
    if (enabledNames !== null) {
      enabled = enabledNames.has(manifest.name) ? "enabled" : "disabled";
    }
    return {
      name: manifest.name,
      spec: manifest.spec,
      pluginDir: manifest.pluginDir,
      install: "installed",
      enabled,
    };
  });
}
