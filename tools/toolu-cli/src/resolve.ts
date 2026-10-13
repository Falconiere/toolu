import { spawnSync } from "node:child_process";
import { accessSync, constants, realpathSync, statSync } from "node:fs";
import { delimiter, dirname, join, resolve, sep } from "node:path";

/**
 * Where the native `toolu` may live, in the order the plugins' generated hook
 * launcher searches (`crates/core/protocol/src/launcher.rs`): `TOOLU_BIN`, the
 * fixed install directories, then every `toolu` on `PATH`.
 */
export function candidates(env: NodeJS.ProcessEnv): string[] {
  const home = env.HOME ?? "";
  const fixed = [
    "/opt/homebrew/bin/toolu",
    "/usr/local/bin/toolu",
    "/home/linuxbrew/.linuxbrew/bin/toolu",
    ...(home === "" ? [] : [join(home, ".local/bin/toolu")]),
  ];
  const onPath = (env.PATH ?? "")
    .split(delimiter)
    .filter((dir) => dir !== "")
    .map((dir) => join(dir, "toolu"));
  const bin = env.TOOLU_BIN ?? "";
  return [...(bin === "" ? [] : [bin]), ...fixed, ...onPath];
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

/** The wrapper itself, a copy inside its own package, or a file that is no program. */
function isWrapper(path: string, self: string): boolean {
  const packageRoot = dirname(dirname(self));
  return (
    path === self ||
    path.startsWith(`${packageRoot}${sep}`) ||
    path.includes(`${sep}node_modules${sep}@toolu${sep}plugins${sep}`)
  );
}

function answers(path: string, args: readonly string[], env: NodeJS.ProcessEnv): string | undefined {
  const result = spawnSync(path, [...args], { encoding: "utf8", env, timeout: 20_000, stdio: ["ignore", "pipe", "ignore"] });
  return result.status === 0 ? result.stdout : undefined;
}

/** What resolution found. */
export type Native =
  | { readonly kind: "found"; readonly path: string }
  | { readonly kind: "old"; readonly path: string }
  | { readonly kind: "missing"; readonly searched: readonly string[] };

/**
 * The native binary: the first candidate that is an executable file, is not
 * this wrapper, prints an integer for `--hook-protocol` and has `plugins install`.
 */
export function findNative(env: NodeJS.ProcessEnv, self: string): Native {
  const searched = candidates(env);
  let old: string | undefined;
  for (const candidate of searched) {
    const path = real(candidate);
    if (path === undefined || isWrapper(path, self) || !executableFile(path)) continue;
    const protocol = answers(path, ["--hook-protocol"], env);
    if (protocol === undefined || !/^\d+\s*$/.test(protocol)) continue;
    if (answers(path, ["plugins", "install", "--help"], env) === undefined) {
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
  return here.endsWith(`${sep}src`)
    ? resolve(here, "../../../.claude-plugin/marketplace.json")
    : resolve(here, "../assets/marketplace.json");
}
