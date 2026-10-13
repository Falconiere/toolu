import { spawnSync } from "node:child_process";
import { accessSync, constants, realpathSync, statSync } from "node:fs";
import { basename, delimiter, dirname, join, resolve, sep } from "node:path";

/** The install directories the plugins' hook launcher searches (`crates/core/protocol/src/launcher.rs`). */
export const FIXED_DIRS: readonly string[] = [
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/home/linuxbrew/.linuxbrew/bin",
];

/** How long one probe of a candidate may take. */
const PROBE_TIMEOUT_MS = 20_000;

/**
 * Where the native `toolu` may live, the way the hook launcher searches: a non-empty
 * `TOOLU_BIN` alone, else the install directories, `~/.local/bin`, then every
 * `toolu` on `PATH`.
 */
export function candidates(env: NodeJS.ProcessEnv, fixed: readonly string[] = FIXED_DIRS): string[] {
  const bin = env.TOOLU_BIN ?? "";
  if (bin !== "") return [bin];
  const home = env.HOME ?? "";
  const dirs = [...fixed, ...(home === "" ? [] : [join(home, ".local/bin")])];
  const onPath = (env.PATH ?? "").split(delimiter).filter((dir) => dir !== "");
  return [...dirs, ...onPath].map((dir) => join(dir, "toolu"));
}

function real(path: string): string | undefined {
  try {
    return realpathSync(path);
  } catch {
    return undefined;
  }
}

function executableFile(path: string): boolean {
  try {
    if (!statSync(path).isFile()) return false;
    accessSync(path, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/**
 * The wrapper itself, or a copy inside its own package: the bundle sits in
 * `<package>/dist/` (published) or `<workspace>/src/` (a checkout), and a
 * global install lives under `node_modules/@toolu/plugins/`.
 */
export function isWrapper(path: string, self: string): boolean {
  const here = dirname(self);
  const inPackage = basename(here) === "dist" || basename(here) === "src";
  return (
    path === self ||
    (inPackage && path.startsWith(`${dirname(here)}${sep}`)) ||
    path.includes(`${sep}node_modules${sep}@toolu${sep}plugins${sep}`)
  );
}

function answers(path: string, args: readonly string[], env: NodeJS.ProcessEnv): string | undefined {
  const result = spawnSync(path, [...args], {
    encoding: "utf8",
    env,
    timeout: PROBE_TIMEOUT_MS,
    stdio: ["ignore", "pipe", "ignore"],
  });
  return result.status === 0 ? result.stdout : undefined;
}

/** What resolution found. */
export type Native =
  | { readonly kind: "found"; readonly path: string }
  | { readonly kind: "old"; readonly path: string }
  | { readonly kind: "missing"; readonly searched: readonly string[] };

/**
 * The native binary: the first candidate that is an executable file, is not
 * this wrapper, prints an integer for `--hook-protocol` and has `plugins
 * install`. Each real path is probed once.
 */
export function findNative(
  env: NodeJS.ProcessEnv,
  self: string,
  fixed: readonly string[] = FIXED_DIRS,
): Native {
  const searched = candidates(env, fixed);
  const tried = new Set<string>();
  let old: string | undefined;
  for (const candidate of searched) {
    const path = real(candidate);
    if (path === undefined || tried.has(path)) continue;
    tried.add(path);
    if (isWrapper(path, self) || !executableFile(path)) continue;
    const protocol = answers(path, ["--hook-protocol"], env);
    if (protocol === undefined || !/^\d+\s*$/.test(protocol)) continue;
    // clap's help always has a usage line; an empty or foreign answer is no `plugins install`.
    const help = answers(path, ["plugins", "install", "--help"], env);
    if (help === undefined || !help.includes("Usage: toolu plugins install")) {
      old ??= path;
      continue;
    }
    return { kind: "found", path };
  }
  return old === undefined ? { kind: "missing", searched } : { kind: "old", path: old };
}

/** The catalog shipped beside the bundle, or the repository's when run from source. */
export function catalogPath(self: string): string {
  const here = dirname(self);
  return basename(here) === "src"
    ? resolve(here, "../../../.claude-plugin/marketplace.json")
    : resolve(here, "../assets/marketplace.json");
}

/** The verbs that take `--catalog`. */
const VERBS = new Set(["install", "list", "remove", "update"]);

/**
 * The words after `toolu`: `plugins`, then the user's, with `--catalog <catalog>`
 * after the first known verb unless the user gave their own `--catalog`.
 */
export function nativeWords(argv: readonly string[], catalog: string | undefined): string[] {
  const own = argv.some((word) => word === "--catalog" || word.startsWith("--catalog="));
  const at = argv.findIndex((word) => VERBS.has(word));
  if (catalog === undefined || own || at < 0) return ["plugins", ...argv];
  return ["plugins", ...argv.slice(0, at + 1), "--catalog", catalog, ...argv.slice(at + 1)];
}
