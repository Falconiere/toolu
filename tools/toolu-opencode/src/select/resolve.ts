/** Enabled plugin set + dependency closure (#211). */
import { pluginSpec } from "../inventory/manifest.ts";
import { listPluginManifests } from "../inventory/scan.ts";
import { resolveEnabledPluginNames } from "../inventory/selection.ts";
import type { PluginManifest } from "../inventory/types.ts";

export type SelectResult =
  | { ok: true; plugins: PluginManifest[] }
  | { ok: false; reason: string; missingDependency?: string };

function manifestByName(manifests: PluginManifest[]): Map<string, PluginManifest> {
  const map = new Map<string, PluginManifest>();
  for (const m of manifests) {
    map.set(m.name, m);
  }
  return map;
}

/** Resolve enabled plugins and transitive manifest dependencies. */
export function selectPluginsWithDependencies(
  pluginsRoot: string,
  projectRoot: string,
): SelectResult {
  const enabledResult = resolveEnabledPluginNames(pluginsRoot, projectRoot);
  if (!enabledResult.ok) {
    return { ok: false, reason: enabledResult.reason };
  }

  const allManifests = listPluginManifests(pluginsRoot);
  if (allManifests === null) {
    return { ok: false, reason: `cannot read plugins root: ${pluginsRoot}` };
  }
  const byName = manifestByName(allManifests);

  const queue = [...enabledResult.enabled];
  const selected = new Set<string>();
  const ordered: PluginManifest[] = [];

  while (queue.length > 0) {
    const name = queue.shift();
    if (!name || selected.has(name)) {
      continue;
    }
    const manifest = byName.get(name);
    if (!manifest) {
      return {
        ok: false,
        reason: `enabled plugin "${name}" is not installed under ${pluginsRoot}`,
        missingDependency: name,
      };
    }
    selected.add(name);
    ordered.push(manifest);
    for (const dep of manifest.dependencies) {
      const depName = dep.name;
      if (!byName.has(depName)) {
        return {
          ok: false,
          reason: `plugin "${manifest.spec}" requires missing dependency ${pluginSpec(dep.name, dep.marketplace)}`,
          missingDependency: depName,
        };
      }
      if (!selected.has(depName)) {
        queue.push(depName);
      }
    }
  }

  return { ok: true, plugins: ordered };
}
