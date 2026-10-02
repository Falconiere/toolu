/**
 * The environment toolu builds on OpenCode (#343), in one place for its three
 * consumers: startup children, in-process gates and the agent's bash.
 *
 * toolu's own processes keep the host env and the user's HOME, lose every
 * other host's root variables (so no Claude or Codex home is ever resolved),
 * and get toolu's roots. The agent's bash keeps its env as the host builds it;
 * `shell.env` only adds toolu's non-secret roots, one root per selected plugin,
 * and Bun's directory when the host PATH has no `bun` for `#!/usr/bin/env bun`
 * helpers. It adds no `TOOLU_PROJECT_DIR`: a helper run in another repository
 * resolves that repository's state from its own git toplevel.
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

export type ShellEnvInput = {
  roots: OpencodeRoots;
  /** The selected plugins, dependencies included. */
  plugins: readonly PluginManifest[];
  /** The resolved Bun executable. */
  bun: string;
  /** The PATH the host gives bash. */
  hostPath: string | undefined;
};

/** What `shell.env` adds to every bash call. Never a value copied from the host env. */
export function shellEnvAdditions(input: ShellEnvInput): Record<string, string> {
  const { roots, plugins, bun, hostPath } = input;
  const env: Record<string, string> = {
    ...rootVars(roots),
    TOOLU_BUN: bun,
    TOOLU_OPENCODE_ROOT: roots.packageRoot,
  };
  const core = plugins.find((plugin) => plugin.name === "toolu");
  if (core !== undefined) env.TOOLU_PLUGIN_ROOT = core.pluginDir;
  for (const plugin of plugins) env[pluginRootVar(plugin.name)] = plugin.pluginDir;
  if (resolveBunExecutable({ PATH: hostPath ?? "" }) === null) {
    env.PATH = hostPath ? `${hostPath}${delimiter}${dirname(bun)}` : dirname(bun);
  }
  return env;
}
