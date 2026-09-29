/**
 * Codex registry prune (#257), a port of `toolu_registry_prune_inactive` that
 * also covers ESM modules. Codex keeps no install record on the hot path, so at
 * SessionStart the registry drops modules whose plugin is definitively absent
 * from the ready plugin snapshot. A missing, stale or malformed snapshot prunes
 * nothing; symlinks and un-namespaced files are never removed.
 */
import { lstatSync, readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { resolveHost, type HostOptions } from "../host/host-roots.ts";
import { codexPluginInstalled } from "../host/host-snapshot.ts";
import { REGISTRY_DIRS, registryRoot } from "./registry-paths.ts";

/** `?*__*.sh` (or `.js`) as `registry.sh` globs it: the spec runs to the first `__`. */
function specOf(file: string): string | undefined {
  if (file.startsWith(".") || !(file.endsWith(".sh") || file.endsWith(".js"))) return undefined;
  const at = file.indexOf("__");
  return at > 0 ? file.slice(0, at) : undefined;
}

function isRegularFile(path: string): boolean {
  try {
    return lstatSync(path).isFile();
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

/** Remove Codex registry modules of plugins absent from the snapshot; returns the removed paths. */
export function pruneInactiveModules(options: HostOptions = {}): string[] {
  const o = resolveHost(options);
  if (o.host !== "codex") return [];
  const absent = new Map<string, boolean>();
  const isAbsent = (spec: string): boolean => {
    const known = absent.get(spec);
    if (known !== undefined) return known;
    const answer = codexPluginInstalled(spec, o) === "absent";
    absent.set(spec, answer);
    return answer;
  };
  const removed: string[] = [];
  const root = registryRoot(o);
  for (const dir of REGISTRY_DIRS) {
    for (const file of readNames(join(root, dir))) {
      const spec = specOf(file);
      const path = join(root, dir, file);
      if (spec === undefined || !isRegularFile(path) || !isAbsent(spec)) continue;
      try {
        rmSync(path);
        removed.push(path);
      } catch {
        // `rm -f` parity: a module that cannot be removed stays gated at dispatch.
      }
    }
  }
  return removed;
}
