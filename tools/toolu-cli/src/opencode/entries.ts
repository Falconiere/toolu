import { CliError, EXIT, UsageError } from "../exit";
import type { ConfigFile } from "./jsonc";

export const PACKAGE = "@toolu/opencode";

/** A toolu element of a file's `plugin` array: a spec string or `[spec, options]`. */
export interface PluginEntry {
  readonly file: string;
  readonly index: number;
  readonly spec: string;
  readonly tuple: boolean;
}

/** npm package name of a plugin spec; URLs and paths come back whole and never match. */
export function packageName(spec: string): string {
  const from = spec.startsWith("@") ? spec.indexOf("/") : 0;
  const at = spec.indexOf("@", Math.max(from, 1));
  return at === -1 ? spec : spec.slice(0, at);
}

/** The version part of a spec (`latest` when unpinned). */
export function versionOf(spec: string): string {
  const name = packageName(spec);
  return spec.length > name.length ? spec.slice(name.length + 1) : "latest";
}

/** The spec install writes: `TOOLU_OPENCODE_PACKAGE`, else the package at the CLI's version. */
export function targetSpec(env: NodeJS.ProcessEnv, version: string): string {
  const override = env.TOOLU_OPENCODE_PACKAGE;
  if (override === undefined || override === "") return `${PACKAGE}@${version}`;
  if (packageName(override) !== PACKAGE) {
    throw new UsageError(`TOOLU_OPENCODE_PACKAGE must name ${PACKAGE}, got: ${override}`);
  }
  return override;
}

/** The file's `plugin` array, or undefined when the key is absent. */
export function pluginArray(file: ConfigFile): readonly unknown[] | undefined {
  const value = file.data?.plugin;
  if (value === undefined) return undefined;
  if (!Array.isArray(value)) {
    throw new CliError(EXIT.failed, `${file.path}: "plugin" must be an array; nothing was written`);
  }
  return value;
}

function specOf(element: unknown): { spec: string; tuple: boolean } | undefined {
  if (typeof element === "string") return { spec: element, tuple: false };
  if (Array.isArray(element) && typeof element[0] === "string") {
    return { spec: element[0], tuple: true };
  }
  return undefined;
}

/** Every element of one file's `plugin` array, in order. */
function elements(file: ConfigFile): readonly (PluginEntry | undefined)[] {
  return (pluginArray(file) ?? []).map((element, index) => {
    const parsed = specOf(element);
    return parsed === undefined ? undefined : { file: file.path, index, ...parsed };
  });
}

/** The toolu entries of one file. */
export function tooluEntries(file: ConfigFile): readonly PluginEntry[] {
  return elements(file).filter(
    (entry): entry is PluginEntry => entry !== undefined && packageName(entry.spec) === PACKAGE,
  );
}

/** The highest-priority global file that defines `plugin` (files in ascending priority). */
export function governingGlobal(global: readonly ConfigFile[]): ConfigFile | undefined {
  return global.toReversed().find((file) => pluginArray(file) !== undefined);
}

/**
 * The toolu entry OpenCode actually loads, mirroring the pinned host: the
 * governing global array, then each project file in load order; an empty array
 * resets the list, and the last entry for a package wins.
 */
export function effectiveEntry(
  global: readonly ConfigFile[],
  project: readonly ConfigFile[],
): PluginEntry | undefined {
  const governing = governingGlobal(global);
  let merged: (PluginEntry | undefined)[] = governing === undefined ? [] : [...elements(governing)];
  for (const file of project) {
    const array = pluginArray(file);
    if (array === undefined) continue;
    merged = array.length === 0 ? [] : [...merged, ...elements(file)];
  }
  return merged.findLast(
    (entry): entry is PluginEntry => entry !== undefined && packageName(entry.spec) === PACKAGE,
  );
}
