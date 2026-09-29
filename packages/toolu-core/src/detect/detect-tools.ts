/**
 * Tool availability (#254): bash `command -v NAME` for an external tool, and
 * `detect_ast_grep`. A PATH scan with no subprocess, cached per PATH and name
 * for the life of the process; a changed PATH is a different key. Shell
 * builtins, functions and aliases are not tools and are never reported.
 */
import { accessSync, constants, statSync } from "node:fs";
import { join } from "node:path";
import { envValue, type HostEnv } from "../host/host-name.ts";

const cache = new Map<string, boolean>();

/**
 * What `command -v` accepts from PATH: an existing entry that is not a
 * directory (symlinks followed). Bash prefers an executable one, but when none
 * exists it still reports the first non-executable file and exits 0, so the
 * execute bit does not change the answer.
 */
function isCommandFile(path: string): boolean {
  try {
    return !statSync(path).isDirectory();
  } catch {
    return false;
  }
}

/** A path given with a `/` must be an executable non-directory: bash has no fallback there. */
function isExecutable(path: string): boolean {
  try {
    accessSync(path, constants.X_OK);
    return !statSync(path).isDirectory();
  } catch {
    return false;
  }
}

function scan(name: string, dirs: readonly string[]): boolean {
  // An empty PATH entry is the current directory, as in bash.
  return dirs.some((dir) => isCommandFile(join(dir === "" ? "." : dir, name)));
}

/** Whether `command -v name` finds a command file. A name holding `/` is checked as a path. */
export function toolAvailable(name: string, env: HostEnv = process.env): boolean {
  if (name === "") return false;
  if (name.includes("/")) return isExecutable(name);
  const path = envValue(env, "PATH") ?? "";
  const dirs = path.split(":");
  // Entries relative to the cwd make the answer cwd-dependent: probe, never cache.
  if (dirs.some((dir) => !dir.startsWith("/"))) return scan(name, dirs);
  const key = `${path}\0${name}`;
  const hit = cache.get(key);
  if (hit !== undefined) return hit;
  const found = scan(name, dirs);
  cache.set(key, found);
  return found;
}

/** `detect_ast_grep`: `sg` or `ast-grep` is on PATH. */
export function detectAstGrep(env: HostEnv = process.env): boolean {
  return toolAvailable("sg", env) || toolAvailable("ast-grep", env);
}
