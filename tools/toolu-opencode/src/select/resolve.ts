/** Enabled plugin set + dependency closure (#211, #345). */
import { pluginSpec } from "../inventory/manifest.ts";
import { listPluginManifests } from "../inventory/scan.ts";
import { resolveEnabledPluginNames, type SelectionSource } from "../inventory/selection.ts";
import type { PluginManifest } from "../inventory/types.ts";

export type SelectResult =
  | { ok: true; plugins: PluginManifest[]; source: SelectionSource; notes: string[] }
  | { ok: false; reason: string; missingDependency?: string };

type Closure =
  | { ok: true; plugins: PluginManifest[] }
  | { ok: false; reason: string; missingDependency?: string };

function manifestByName(manifests: PluginManifest[]): Map<string, PluginManifest> {
  const map = new Map<string, PluginManifest>();
  for (const m of manifests) {
    map.set(m.name, m);
  }
  return map;
}

function closePluginDependencies(
  pluginsRoot: string,
  byName: Map<string, PluginManifest>,
  seedNames: Iterable<string>,
): Closure {
  const queue = [...seedNames];
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
      if (!byName.has(dep.name)) {
        return {
          ok: false,
          reason: `plugin "${manifest.spec}" requires missing dependency ${pluginSpec(dep.name, dep.marketplace)}`,
          missingDependency: dep.name,
        };
      }
      if (!selected.has(dep.name)) {
        queue.push(dep.name);
      }
    }
  }

  return { ok: true, plugins: ordered };
}

/**
 * Resolve enabled plugins and transitive manifest dependencies. With
 * `globalConfigRoot`, a global selection file applies when the project has none.
 */
export function selectPluginsWithDependencies(
  pluginsRoot: string,
  projectRoot: string,
  globalConfigRoot?: string,
): SelectResult {
  const enabledResult = resolveEnabledPluginNames(pluginsRoot, projectRoot, globalConfigRoot);
  if (!enabledResult.ok) {
    return { ok: false, reason: enabledResult.reason };
  }

  const allManifests = listPluginManifests(pluginsRoot);
  if (allManifests === null) {
    return { ok: false, reason: `cannot read plugins root: ${pluginsRoot}` };
  }
  const closed = closePluginDependencies(
    pluginsRoot,
    manifestByName(allManifests),
    enabledResult.enabled,
  );
  if (!closed.ok) return closed;
  const { source, path, unknown } = enabledResult;
  // Unknown names come only from an explicit selection file, which always has a path.
  const notes =
    path === undefined
      ? []
      : unknown.map((name) => `enabled plugin "${name}" in ${path} is not installed`);
  return { ...closed, source, notes };
}

/** Resolve an explicit enabled list plus manifest dependency closure (#211). */
export function selectPluginsByEnabledNames(
  pluginsRoot: string,
  enabledNames: readonly string[],
): SelectResult {
  const allManifests = listPluginManifests(pluginsRoot);
  if (allManifests === null) {
    return { ok: false, reason: `cannot read plugins root: ${pluginsRoot}` };
  }
  const closed = closePluginDependencies(pluginsRoot, manifestByName(allManifests), enabledNames);
  if (!closed.ok) return closed;
  return { ...closed, source: "default", notes: [] };
}
