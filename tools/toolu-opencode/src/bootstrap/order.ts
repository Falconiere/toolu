/**
 * Startup order (#342): dependencies before dependents, deterministic. A depth
 * first walk over names and dependencies in sorted order; a dependency cycle
 * or a dependency outside the given set is a reason, never a partial order.
 */
import type { PluginManifest } from "../inventory/types.ts";

export type OrderResult = { ok: true; plugins: PluginManifest[] } | { ok: false; reason: string };

type Walk = {
  byName: ReadonlyMap<string, PluginManifest>;
  done: Set<string>;
  path: string[];
  out: PluginManifest[];
};

function sortedNames(names: Iterable<string>): string[] {
  return [...names].toSorted();
}

/** Visit `name` and its dependencies; returns why the order cannot exist, if it cannot. */
function visit(name: string, walk: Walk): string | undefined {
  if (walk.done.has(name)) return undefined;
  const at = walk.path.indexOf(name);
  if (at >= 0) return `plugin dependency cycle: ${[...walk.path.slice(at), name].join(" -> ")}`;
  const plugin = walk.byName.get(name);
  if (plugin === undefined) return `plugin ${name} is not selected`;
  walk.path.push(name);
  for (const dep of sortedNames(plugin.dependencies.map((d) => d.name))) {
    if (!walk.byName.has(dep)) return `${name} requires unselected dependency ${dep}`;
    const reason = visit(dep, walk);
    if (reason !== undefined) return reason;
  }
  walk.path.pop();
  walk.done.add(name);
  walk.out.push(plugin);
  return undefined;
}

export function startupOrder(plugins: readonly PluginManifest[]): OrderResult {
  const byName = new Map(plugins.map((plugin) => [plugin.name, plugin]));
  const walk: Walk = { byName, done: new Set(), path: [], out: [] };
  for (const name of sortedNames(byName.keys())) {
    const reason = visit(name, walk);
    if (reason !== undefined) return { ok: false, reason };
  }
  return { ok: true, plugins: walk.out };
}
