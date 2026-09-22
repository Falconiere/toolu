/** OpenCode config/state roots; honors TOOLU_* without Claude/Codex homes (#211). */
import { join } from "node:path";
import type { OpencodeRootsOptions } from "./types.ts";

const OPENCODE_PROJECT_DIRNAME = ".opencode";
const OPENCODE_STATE_SEGMENT = join("toolu", "state");

/** Writable data root for assembled registry and host state. */
export function opencodeDataRoot(options: OpencodeRootsOptions = {}): string {
  const env = options.env ?? process.env;
  if (options.dataRoot) {
    return options.dataRoot;
  }
  if (env.TOOLU_CONFIG_DIR) {
    return env.TOOLU_CONFIG_DIR;
  }
  if (env.TOOLU_OPENCODE_HOME) {
    return env.TOOLU_OPENCODE_HOME;
  }
  const projectRoot = options.projectRoot ?? env.TOOLU_PROJECT_DIR;
  if (!projectRoot) {
    throw new Error(
      "opencodeDataRoot: set dataRoot, TOOLU_CONFIG_DIR, TOOLU_OPENCODE_HOME, or projectRoot",
    );
  }
  return join(projectRoot, OPENCODE_PROJECT_DIRNAME, OPENCODE_STATE_SEGMENT);
}

/** Alias for portable-core export map. */
export function opencodeConfigRoot(options: OpencodeRootsOptions = {}): string {
  return opencodeDataRoot(options);
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
