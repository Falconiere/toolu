import { UsageError } from "../exit";
import type { CatalogEntry, Marketplace } from "./types";

/** Names declared by the manifest, in declaration order. */
export function catalogNames(marketplace: Marketplace): readonly string[] {
  return marketplace.plugins.map((plugin) => plugin.name);
}

function dependencyNames(entry: CatalogEntry): readonly string[] {
  return (entry.dependencies ?? []).map((dependency) => dependency.name);
}

interface Walk {
  readonly entries: ReadonlyMap<string, CatalogEntry>;
  readonly ordered: string[];
  readonly seen: Set<string>;
  readonly active: Set<string>;
}

function visit(name: string, walk: Walk): void {
  if (walk.seen.has(name)) return;
  if (walk.active.has(name)) throw new UsageError(`circular plugin dependency involving ${name}`);
  const entry = walk.entries.get(name);
  if (entry === undefined) {
    throw new UsageError(`unknown plugin: ${name}. Run \`toolu plugins list\` for the catalog.`);
  }
  walk.active.add(name);
  for (const dependency of dependencyNames(entry)) {
    if (walk.entries.has(dependency)) visit(dependency, walk);
  }
  walk.active.delete(name);
  walk.seen.add(name);
  walk.ordered.push(name);
}

/**
 * Orders a requested selection so every plugin follows the plugins it depends on.
 * An empty request means the whole catalog. Unknown names are rejected.
 */
export function installOrder(
  marketplace: Marketplace,
  requested: readonly string[],
): readonly string[] {
  const walk: Walk = {
    entries: new Map(marketplace.plugins.map((plugin) => [plugin.name, plugin])),
    ordered: [],
    seen: new Set<string>(),
    active: new Set<string>(),
  };
  for (const name of requested.length > 0 ? requested : catalogNames(marketplace)) {
    visit(name, walk);
  }
  return walk.ordered;
}

/** Plugins the catalog declares as depending on the given plugin. */
export function dependentsOf(marketplace: Marketplace, name: string): readonly string[] {
  return marketplace.plugins
    .filter((plugin) => dependencyNames(plugin).includes(name))
    .map((plugin) => plugin.name);
}
