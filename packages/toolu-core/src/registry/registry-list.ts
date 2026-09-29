/**
 * Registry directory listing (#257). Order and admission match `dispatch.sh`
 * under `LC_ALL=C`: byte order, dotfiles hidden as a glob hides them, regular
 * files or symlinks to them only, and a name without the `<spec>__` namespace
 * rejected rather than run.
 */
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import type { HostOptions } from "../host/host-roots.ts";
import { parseRegistryName, registryEventDir, type ParsedName } from "./registry-paths.ts";
import type { RegistryEvent } from "./registry-types.ts";

export type RegistryEntry = ParsedName & { path: string; file: string };

export type RegistryListing = {
  entries: RegistryEntry[];
  /** Module files lacking the `<spec>__<name>` namespace, never run. */
  rejected: string[];
};

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

function readNames(dir: string): string[] {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

/** Every `.js` and `.sh` module file in `dir`, in byte order; an absent directory is empty. */
export function listRegistryDir(dir: string): RegistryListing {
  const listing: RegistryListing = { entries: [], rejected: [] };
  const names = readNames(dir).toSorted((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  for (const file of names) {
    if (file.startsWith(".")) continue;
    const parsed = parseRegistryName(file);
    const path = join(dir, file);
    if (parsed === undefined || !isFile(path)) continue;
    if (parsed === null) listing.rejected.push(file);
    else listing.entries.push({ ...parsed, path, file });
  }
  return listing;
}

/** The listing of `event`'s registry directory. */
export function listRegistryModules(
  event: RegistryEvent,
  options: HostOptions = {},
): RegistryListing {
  return listRegistryDir(registryEventDir(event, options));
}
