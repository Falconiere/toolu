/**
 * The environment toolu builds on OpenCode (#343), in one place for its three
 * consumers: startup children, in-process gates and the agent's bash.
 *
 * toolu's own processes keep the host env and the user's HOME, lose every
 * other host's root variables (so no Claude or Codex home is ever resolved),
 * and get toolu's roots. The agent's bash keeps its env as the host builds it;
 * `shell.env` only adds toolu's non-secret roots, one root per selected plugin,
 * and Bun's directory when PATH has no `bun` for `#!/usr/bin/env bun` helpers.
 * It sets no `TOOLU_PROJECT_DIR` (and blanks an exported one): a helper run in
 * another repository resolves that repository's state from its own toplevel.
 */
import { delimiter, dirname, join } from "node:path";
import type { PluginManifest } from "../inventory/types.ts";
import { resolveBunExecutable } from "../preflight/check.ts";

export type OpencodeRoots = {
  /** The host instance's worktree. */
  projectRoot: string;
  /** Per-project registry, helpers and startup ledger. */
  dataRoot: string;
  /** Global `toolu.config.json` directory. */
  userConfigRoot: string;
  /** Parent of the plugin catalog (`<repoRoot>/plugins`). */
  repoRoot: string;
  /** The `@toolu/opencode` package directory, which holds `generated/`. */
  packageRoot: string;
};

/** The roots toolu's own processes need; the package root matters only to bash. */
export type ProcessRoots = Omit<OpencodeRoots, "packageRoot">;

/** Root variables of the other hosts; none may steer toolu on OpenCode. */
export const FOREIGN_HOST_VARS = [
  "CLAUDE_CONFIG_DIR",
  "CLAUDE_PROJECT_DIR",
  "CLAUDE_PLUGIN_ROOT",
  "CLAUDE_PLUGIN_DATA",
  "CLAUDE_PLUGINS_REGISTRY",
  "CODEX_HOME",
  "PLUGIN_ROOT",
  "PLUGIN_DATA",
  "TOOLU_CODEX_PLUGIN_SNAPSHOT",
  "CURSOR_PROJECT_DIR",
  "CURSOR_PLUGIN_ROOT",
  "HERMES_HOME",
] as const;

const FOREIGN = new Set<string>(FOREIGN_HOST_VARS);

/** `env` without its unset keys. */
export function definedEnv(env: NodeJS.ProcessEnv): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value;
  }
  return out;
}

export function withoutForeignHostVars(env: Record<string, string>): Record<string, string> {
  return Object.fromEntries(Object.entries(env).filter(([key]) => !FOREIGN.has(key)));
}

/** Where every toolu process on OpenCode, and every helper the agent runs, finds toolu. */
function rootVars(roots: ProcessRoots): Record<string, string> {
  return {
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: roots.dataRoot,
    TOOLU_USER_CONFIG_DIR: roots.userConfigRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_SETTINGS_DIR: join(roots.repoRoot, "plugins", "toolu", "settings"),
  };
}

/** The env of toolu's own startup children and gates: the host's, HOME included, re-rooted. */
export function tooluProcessEnv(
  base: Record<string, string>,
  roots: ProcessRoots,
): Record<string, string> {
  return {
    ...withoutForeignHostVars(base),
    ...rootVars(roots),
    TOOLU_PROJECT_DIR: roots.projectRoot,
  };
}

/** `epic-orchestrator` → `TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR`. */
export function pluginRootVar(name: string): string {
  return `TOOLU_PLUGIN_ROOT_${name.toUpperCase().replaceAll("-", "_")}`;
}

/**
 * Marks `TOOLU_CONFIG_DIR` in bash as the session's own data root, so an
 * OpenCode started from that bash treats it as inherited, not as an override.
 */
export const DATA_ROOT_MARKER = "TOOLU_OPENCODE_DATA_ROOT";

export type ShellEnvInput = {
  roots: OpencodeRoots;
  /** The selected plugins, dependencies included. */
  plugins: readonly PluginManifest[];
  /** The resolved Bun executable. */
  bun: string;
  /** The host env bash starts from. */
  host: Readonly<Record<string, string>>;
};

/** What `shell.env` gives every bash call: fixed variables, and Bun for PATH when it lacks one. */
export type ShellEnv = {
  vars: Record<string, string>;
  bun: string;
  hostPath: string | undefined;
};

/** `shell.env`'s contribution. Never a value copied from the host env. */
export function shellEnvFor(input: ShellEnvInput): ShellEnv {
  const { roots, plugins, bun, host } = input;
  const vars: Record<string, string> = {
    ...rootVars(roots),
    [DATA_ROOT_MARKER]: roots.dataRoot,
    TOOLU_BUN: bun,
    TOOLU_OPENCODE_ROOT: roots.packageRoot,
  };
  // An exported project dir would point every helper at one project; blank is unset to core.
  if ((host.TOOLU_PROJECT_DIR ?? "") !== "") vars.TOOLU_PROJECT_DIR = "";
  const core = plugins.find((plugin) => plugin.name === "toolu");
  if (core !== undefined) vars.TOOLU_PLUGIN_ROOT = core.pluginDir;
  for (const plugin of plugins) vars[pluginRootVar(plugin.name)] = plugin.pluginDir;
  return { vars, bun, hostPath: host.PATH };
}

/**
 * Apply `shell` to the env one bash call is being built with. PATH gains Bun's
 * directory, last, only when the PATH bash will see (an earlier plugin's, else
 * the host's) has no `bun`, so `#!/usr/bin/env bun` helpers run.
 */
export function applyShellEnv(shell: ShellEnv, env: Record<string, string>): void {
  Object.assign(env, shell.vars);
  const path = env.PATH ?? shell.hostPath;
  if (resolveBunExecutable({ PATH: path ?? "" }) !== null) return;
  const bunDir = dirname(shell.bun);
  env.PATH = path ? `${path}${delimiter}${bunDir}` : bunDir;
}
