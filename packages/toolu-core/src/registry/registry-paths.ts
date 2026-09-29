/**
 * Registry locations and file names (#257), a port of `registry.sh`: the root is
 * `<config root>/toolu`, and each event keeps the directory name bash uses.
 */
import { join } from "node:path";
import { configRoot, type HostOptions } from "../host/host-roots.ts";
import type { RegistryEvent } from "./registry-types.ts";

const EVENT_DIRS: Readonly<Record<RegistryEvent, string>> = {
  "tool/pre": "pre-tools.d",
  "tool/post": "post-tools.d",
};

export const REGISTRY_DIRS: readonly string[] = Object.values(EVENT_DIRS);

/** The separator between spec and name; specs may contain `.` but never `__`. */
const SEP = "__";

/** `<config root>/toolu` (not created). */
export function registryRoot(options: HostOptions = {}): string {
  return join(configRoot(options), "toolu");
}

/** `pre-tools.d` or `post-tools.d`. */
export function registryDirName(event: RegistryEvent): string {
  return EVENT_DIRS[event];
}

/** `<root>/pre-tools.d` or `<root>/post-tools.d` (not created). */
export function registryEventDir(event: RegistryEvent, options: HostOptions = {}): string {
  return join(registryRoot(options), EVENT_DIRS[event]);
}

/** `<spec>__<name>.js`; throws on a spec or name that could not round-trip. */
export function registryFileName(spec: string, name: string): string {
  if (spec === "" || /\s/u.test(spec) || spec.includes("/") || spec.includes(SEP)) {
    throw new TypeError(`registry: invalid plugin spec ${JSON.stringify(spec)}`);
  }
  if (name === "" || name.includes("/") || name.startsWith(".")) {
    throw new TypeError(`registry: invalid module name ${JSON.stringify(name)}`);
  }
  return `${spec}${SEP}${name}.js`;
}

export type ParsedName = { spec: string; name: string; kind: "esm" | "bash" };

/**
 * Split a registry base name the way `dispatch.sh` does: the spec runs to the
 * first `__` and must be non-empty and whitespace-free. `undefined` for a
 * name that is not a `.js` or `.sh` module, `null` for one that is but lacks
 * the namespace.
 */
export function parseRegistryName(base: string): ParsedName | null | undefined {
  let kind: ParsedName["kind"];
  if (base.endsWith(".js")) kind = "esm";
  else if (base.endsWith(".sh")) kind = "bash";
  else return undefined;
  const stem = base.slice(0, -3);
  const at = stem.indexOf(SEP);
  if (at <= 0) return null;
  const spec = stem.slice(0, at);
  if (/\s/u.test(spec)) return null;
  return { spec, name: stem.slice(at + SEP.length), kind };
}
