/**
 * OpenCode roots (#211, #343). Three roots, never a Claude or Codex home:
 * - project: the host instance's worktree; holds `.opencode/toolu.config.json`,
 *   the plugin selection and gate state.
 * - global config: `TOOLU_CONFIG_DIR`, then `TOOLU_OPENCODE_HOME`, then
 *   OpenCode's own config directory (`$XDG_CONFIG_HOME/opencode`, else
 *   `~/.config/opencode`); holds the global `toolu.config.json`, read only.
 * - data: registry modules, helpers and the startup ledger, one per project.
 *   With an override set, several projects share the override, so each gets a
 *   keyed directory under it and none can prune or relink another's files.
 */
import { createHash } from "node:crypto";
import { existsSync, realpathSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join, resolve } from "node:path";
import type { OpencodeRootsOptions } from "./types.ts";

const OPENCODE_PROJECT_DIRNAME = ".opencode";
const OPENCODE_STATE_SEGMENT = join("toolu", "state");
const KEYED_PROJECTS_SEGMENT = join("toolu", "opencode", "projects");
const SLUG_CHARS = 32;
const HASH_CHARS = 16;

function nonEmpty(value: string | undefined): string | undefined {
  return value !== undefined && value.length > 0 ? value : undefined;
}

/** The explicit cross-project override, if any: `TOOLU_CONFIG_DIR`, then `TOOLU_OPENCODE_HOME`. */
function override(env: NodeJS.ProcessEnv): string | undefined {
  return nonEmpty(env.TOOLU_CONFIG_DIR) ?? nonEmpty(env.TOOLU_OPENCODE_HOME);
}

/** Global config root: the override, else OpenCode's XDG config directory. Never throws. */
export function opencodeConfigRoot(options: OpencodeRootsOptions = {}): string {
  const env = options.env ?? process.env;
  const xdg = nonEmpty(env.XDG_CONFIG_HOME) ?? join(nonEmpty(env.HOME) ?? homedir(), ".config");
  return override(env) ?? join(xdg, "opencode");
}

function realOrResolved(path: string): string {
  try {
    return realpathSync(path);
  } catch {
    return resolve(path);
  }
}

/** `<slug>-<16 hex>`: the project's basename, readable, plus a hash of its real path. */
export function opencodeProjectKey(projectRoot: string): string {
  const real = realOrResolved(projectRoot);
  // Runs collapse to one `-` first, so trimming needs no repetition.
  const slug = basename(real)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, SLUG_CHARS)
    .replace(/-$/, "");
  const hash = createHash("sha256").update(real).digest("hex").slice(0, HASH_CHARS);
  return `${slug === "" ? "project" : slug}-${hash}`;
}

/** Writable data root for registry modules, helpers and the startup ledger. */
export function opencodeDataRoot(options: OpencodeRootsOptions = {}): string {
  if (options.dataRoot) {
    return options.dataRoot;
  }
  const env = options.env ?? process.env;
  const projectRoot = options.projectRoot;
  if (!projectRoot) {
    throw new Error("opencodeDataRoot: set dataRoot or projectRoot");
  }
  const shared = override(env);
  if (shared !== undefined) {
    return join(shared, KEYED_PROJECTS_SEGMENT, opencodeProjectKey(projectRoot));
  }
  return join(projectRoot, OPENCODE_PROJECT_DIRNAME, OPENCODE_STATE_SEGMENT);
}

/** Project git/worktree root for config resolution. */
export function opencodeProjectDir(options: OpencodeRootsOptions = {}): string {
  const env = options.env ?? process.env;
  if (options.projectRoot) {
    return options.projectRoot;
  }
  if (env.TOOLU_PROJECT_DIR) {
    return env.TOOLU_PROJECT_DIR;
  }
  throw new Error("opencodeProjectDir: set projectRoot or TOOLU_PROJECT_DIR");
}

/** Project-scoped toolu.config.json path under .opencode/. */
export function opencodeProjectConfigPath(projectRoot: string): string {
  return join(projectRoot, OPENCODE_PROJECT_DIRNAME, "toolu.config.json");
}

/** Selection file for enabled plugins (never Claude installed_plugins.json). */
export function opencodePluginSelectionPath(projectRoot: string): string {
  return join(projectRoot, OPENCODE_PROJECT_DIRNAME, "toolu", "plugins.json");
}

/** Registry root under the OpenCode data root. */
export function opencodeRegistryRoot(dataRoot: string): string {
  return join(dataRoot, "toolu");
}

/**
 * `<override>/toolu` when it still holds the startup ledger that releases
 * before #343 shared across every project under an override; else undefined.
 */
export function opencodeLegacySharedRoot(options: OpencodeRootsOptions = {}): string | undefined {
  const shared = override(options.env ?? process.env);
  if (shared === undefined) return undefined;
  const root = opencodeRegistryRoot(shared);
  return existsSync(join(root, "startup-ledger.json")) ? root : undefined;
}
