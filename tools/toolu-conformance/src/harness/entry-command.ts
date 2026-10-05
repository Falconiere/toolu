/** Resolve a committed hook entry to its Bun bundle or selected Rust CLI command. */
import { accessSync, existsSync, readFileSync, statSync, constants } from "node:fs";
import { basename, join, resolve } from "node:path";
import { launcherCommand, type LauncherTarget } from "@toolu/core/launcher";
import { z } from "zod";

export type EntryCommand = {
  plugin: string;
  entry: string;
  bundle: string;
  defaultArgv?: string[];
};

export type ResolvedCommand = {
  argv: string[];
  implementation: "bun" | "rust";
};

type SelectorEnv = Record<string, string | undefined>;
const REPO_ROOT = resolve(import.meta.dir, "../../../..");
const ENTRY_NAME = /^[a-z0-9][a-z0-9-]*$/;

function selectedEntries(value: string | undefined): Set<string> | "all" | null {
  if (value === undefined || value === "") return null;
  if (value === "rust") return "all";
  if (!value.startsWith("rust:")) {
    throw new Error(`invalid TOOLU_IMPL: ${value}`);
  }
  const entries = value.slice(5).split(",");
  const selected = new Set<string>();
  for (const item of entries) {
    const parts = item.split("/");
    if (
      parts.length !== 2 ||
      !ENTRY_NAME.test(parts[0] ?? "") ||
      !ENTRY_NAME.test(parts[1] ?? "") ||
      selected.has(item)
    ) {
      throw new Error(`invalid TOOLU_IMPL: ${value}`);
    }
    selected.add(item);
  }
  return selected;
}

/** Whether this exact entry is selected; parses the selector without probing the binary. */
export function entryImplementation(
  plugin: string,
  entry: string,
  env: SelectorEnv = process.env,
): "bun" | "rust" {
  const selected = selectedEntries(env.TOOLU_IMPL);
  return selected === "all" || selected?.has(`${plugin}/${entry}`) ? "rust" : "bun";
}

/**
 * A test-name suffix naming the Rust implementation when the selector runs it
 * for this entry, so a failing case says which implementation failed; "" otherwise.
 */
export function implementationTag(
  plugin: string,
  entry: string,
  env: SelectorEnv = process.env,
): string {
  return entryImplementation(plugin, entry, env) === "rust" ? ` [rust:${plugin}/${entry}]` : "";
}

function rustBinary(plugin: string, entry: string, env: SelectorEnv): string {
  const dir = env.TOOLU_RUST_BIN_DIR;
  if (dir === "") throw new Error("TOOLU_RUST_BIN_DIR must not be empty");
  const root = dir === undefined ? join(REPO_ROOT, "target", "release") : resolve(dir);
  const binary = join(root, "toolu");
  const label = `rust:${plugin}/${entry}`;
  let info;
  try {
    info = statSync(binary);
  } catch (error: unknown) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`${label}: binary not found at ${binary}`, { cause: error });
    }
    throw error;
  }
  if (!info.isFile()) throw new Error(`${label}: binary is not a file at ${binary}`);
  try {
    accessSync(binary, constants.X_OK);
  } catch (error: unknown) {
    throw new Error(`${label}: binary is not executable at ${binary}`, { cause: error });
  }
  return binary;
}

/** The repository's own root for `plugin`. */
export function pluginRoot(plugin: string): string {
  return join(REPO_ROOT, "plugins", plugin);
}

const PluginManifest = z.looseObject({ name: z.string() });

/**
 * The plugin a root holds, by its `.claude-plugin/plugin.json` name, so a copy
 * in a sandbox directory keeps its identity; the directory name otherwise.
 */
export function pluginName(root: string): string {
  const manifest = join(root, ".claude-plugin", "plugin.json");
  if (!existsSync(manifest)) return basename(root);
  return PluginManifest.parse(JSON.parse(readFileSync(manifest, "utf8"))).name;
}

/** The committed bundle for `entry` under `root`; a relative root gives a plugin-relative path. */
export function bundlePath(root: string, entry: string): string {
  return join(root, "hooks", "dist", `${entry}.js`);
}

/**
 * argv for `plugin`'s `entry`: `bun <bundle>` by default, the selected Rust
 * command under `TOOLU_IMPL`. `root` defaults to the repository's plugin
 * directory; pass a copied plugin root to run that copy's bundle.
 */
export function entryArgv(
  plugin: string,
  entry: string,
  root: string = pluginRoot(plugin),
  env: SelectorEnv = process.env,
): string[] {
  return resolveEntryCommand({ plugin, entry, bundle: resolve(bundlePath(root, entry)) }, env).argv;
}

/**
 * argv for a skill CLI bundle run by path through its shebang, the way its
 * published symlink runs it; the selected Rust command under `TOOLU_IMPL`.
 */
export function publishedArgv(
  plugin: string,
  entry: string,
  root: string = pluginRoot(plugin),
  env: SelectorEnv = process.env,
): string[] {
  const bundle = resolve(bundlePath(root, entry));
  return resolveEntryCommand({ plugin, entry, bundle, defaultArgv: [bundle] }, env).argv;
}

/** Return argv without a shell; selected binary setup errors occur before spawn. */
export function resolveEntryCommand(
  command: EntryCommand,
  env: SelectorEnv = process.env,
): ResolvedCommand {
  const { plugin, entry, bundle, defaultArgv } = command;
  if (entryImplementation(plugin, entry, env) === "bun") {
    return { argv: defaultArgv ?? [process.execPath, bundle], implementation: "bun" };
  }
  // Only a selected entry becomes CLI arguments; a Bun run keeps any copied root.
  if (!ENTRY_NAME.test(plugin) || !ENTRY_NAME.test(entry)) {
    throw new Error(`invalid hook entry: ${plugin}/${entry} at ${bundle}`);
  }
  const args = plugin === "toolu" ? ["hook", entry] : [plugin, "hook", entry];
  return { argv: [rustBinary(plugin, entry, env), ...args], implementation: "rust" };
}

/**
 * argv for a hook the way its host runs it: the plugin's hooks.json launcher
 * under `sh -c` by default, the selected Rust command under `TOOLU_IMPL`.
 */
export function launchedArgv(
  target: LauncherTarget,
  root: string = pluginRoot(target.plugin),
  env: SelectorEnv = process.env,
): string[] {
  return resolveEntryCommand(
    {
      plugin: target.plugin,
      entry: target.entry,
      bundle: resolve(bundlePath(root, target.entry)),
      defaultArgv: ["/bin/sh", "-c", launcherCommand(target)],
    },
    env,
  ).argv;
}
