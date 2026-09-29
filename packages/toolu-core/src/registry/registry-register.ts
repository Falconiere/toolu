/**
 * SessionStart registry sync (#257), replacing each plugin's `register.sh`. A
 * plugin declares its modules, each one committed bundle; `registerModules`
 * copies them into the registry atomically, and removes every entry under the
 * plugin's own `<spec>__` prefix that is no longer a module, including the
 * `.sh` a plugin shipped before its port and crashed writers' tmp residue.
 * Other plugins' entries are never touched. Sync failures are reported, not
 * thrown: a failed sync leaves the registry copy stale, not broken. Only an
 * invalid spec or module name throws, before anything is written.
 */
import {
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import type { HostOptions } from "../host/host-roots.ts";
import {
  REGISTRY_DIRS,
  registryDirName,
  registryFileName,
  registryRoot,
} from "./registry-paths.ts";
import type { RegistryEvent } from "./registry-types.ts";

/** One module a plugin contributes: its committed bundle and the event it serves. */
export type RegisterModuleSpec = { name: string; event: RegistryEvent; bundle: string };

export type RegisterResult = {
  written: string[];
  unchanged: string[];
  pruned: string[];
  failed: { path: string; error: string }[];
};

export type RegisterOptions = HostOptions & { now?: () => number };

/** `find -mmin +1`: a concurrent SessionStart's in-flight tmp is seconds old and is left alone. */
const RESIDUE_AGE_MS = 60_000;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function removeQuietly(path: string): void {
  try {
    rmSync(path, { force: true });
  } catch {
    // Litter at worst: a later run sweeps `<target>.tmp.*` residue by age.
  }
}

function readOrUndefined(path: string): Buffer | undefined {
  try {
    return readFileSync(path);
  } catch {
    return undefined;
  }
}

/** Copy `bundle` to `target` through `<target>.tmp.<pid>` unless the bytes already match. */
function syncModule(bundle: string, target: string, result: RegisterResult): void {
  let bytes: Buffer;
  try {
    bytes = readFileSync(bundle);
  } catch (error) {
    // Keep whatever copy the registry holds: stale enforcement beats none.
    result.failed.push({ path: target, error: `bundle unreadable: ${message(error)}` });
    return;
  }
  if (readOrUndefined(target)?.equals(bytes) === true) {
    result.unchanged.push(target);
    return;
  }
  const tmp = `${target}.tmp.${String(process.pid)}`;
  try {
    mkdirSync(dirname(target), { recursive: true });
    // The tmp name is predictable: drop whatever holds it, then create it
    // exclusively, so a planted symlink is replaced rather than written through.
    removeQuietly(tmp);
    writeFileSync(tmp, bytes, { flag: "wx" });
    renameSync(tmp, target);
    result.written.push(target);
  } catch (error) {
    removeQuietly(tmp);
    result.failed.push({ path: target, error: message(error) });
  }
}

/** `[ -f ]`: a regular file, or a symlink to one. */
function isFileFollowed(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** What `find -name … -delete` removes without following links: a file or a link. */
function isFileOrLink(path: string): boolean {
  try {
    const stat = lstatSync(path);
    return stat.isFile() || stat.isSymbolicLink();
  } catch {
    return false;
  }
}

function ageMs(path: string, now: number): number {
  try {
    return now - lstatSync(path).mtimeMs;
  } catch {
    return 0;
  }
}

function remove(path: string, result: RegisterResult): void {
  try {
    rmSync(path);
    result.pruned.push(path);
  } catch (error) {
    result.failed.push({ path, error: message(error) });
  }
}

/** Remove our stale modules and old tmp residue from one registry directory. */
function pruneDir(
  dir: string,
  prefix: string,
  keep: ReadonlySet<string>,
  now: number,
  result: RegisterResult,
): void {
  let names: string[];
  try {
    names = readdirSync(dir);
  } catch {
    return;
  }
  for (const file of names.filter((name) => name.startsWith(prefix))) {
    const path = join(dir, file);
    if (/\.(?:js|sh)\.tmp\./u.test(file)) {
      if (isFileOrLink(path) && ageMs(path, now) > RESIDUE_AGE_MS) remove(path, result);
    } else if ((file.endsWith(".js") || file.endsWith(".sh")) && !keep.has(path)) {
      // `[ -f ]` parity: a symlink to a file goes; a dangling link or a directory stays.
      if (isFileFollowed(path)) remove(path, result);
    }
  }
}

/**
 * Sync `spec`'s modules into `<config>/toolu/<dir>.d/<spec>__<name>.js`.
 * Throws `TypeError` only for an invalid spec or name, before touching disk.
 */
export function registerModules(
  spec: string,
  modules: readonly RegisterModuleSpec[],
  options: RegisterOptions = {},
): RegisterResult {
  const root = registryRoot(options);
  const planned = modules.map((m) => ({
    bundle: m.bundle,
    target: join(root, registryDirName(m.event), registryFileName(spec, m.name)),
  }));
  const result: RegisterResult = { written: [], unchanged: [], pruned: [], failed: [] };
  for (const { bundle, target } of planned) syncModule(bundle, target, result);
  const keep = new Set(planned.map((p) => p.target));
  const now = options.now?.() ?? Date.now();
  for (const dir of REGISTRY_DIRS) pruneDir(join(root, dir), `${spec}__`, keep, now, result);
  return result;
}

/**
 * A plugin's SessionStart entry: drain stdin so the host never stalls on the
 * pipe, sync, and stay silent on stdout (SessionStart stdout becomes context).
 * Each failure is one stderr line. Never rejects: the hook always exits 0.
 */
export async function runRegisterHook(
  spec: string,
  modules: readonly RegisterModuleSpec[],
  options: RegisterOptions = {},
): Promise<void> {
  const warn = (line: string): void => {
    process.stderr.write(`toolu-registry: register ${spec}: ${line}\n`);
  };
  try {
    await Bun.stdin.text();
    const result = registerModules(spec, modules, options);
    for (const failure of result.failed) warn(`${failure.path}: ${failure.error}`);
  } catch (error) {
    warn(message(error));
  }
}
