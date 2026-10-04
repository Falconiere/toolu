import { homedir } from "node:os";
import { join, resolve } from "node:path";
import {
  opencodeConfigRoot,
  opencodeGlobalPluginSelectionPath,
  opencodePluginSelectionPath,
  opencodeProjectConfigPath,
} from "@toolu/opencode/host";
import { run } from "../host/run";

export type OpencodeScope = "global" | "project";

/** Every file the OpenCode verbs read or write for one project. */
export interface OpencodePaths {
  readonly projectRoot: string;
  /** Global config files in ascending priority: the last one defining `plugin` wins. */
  readonly globalFiles: readonly string[];
  /** Worktree-root config files in the host's load order. */
  readonly projectFiles: readonly string[];
  readonly selection: Readonly<Record<OpencodeScope, string>>;
  readonly tooluConfig: string;
}

function nonEmpty(value: string | undefined): string | undefined {
  return value !== undefined && value.length > 0 ? value : undefined;
}

/** OpenCode's own config directory; toolu's overrides never move it. */
function hostConfigDir(env: NodeJS.ProcessEnv): string {
  const base = nonEmpty(env.XDG_CONFIG_HOME) ?? join(nonEmpty(env.HOME) ?? homedir(), ".config");
  return resolve(base, "opencode");
}

/** The git worktree containing `cwd`, else `cwd` itself. */
export async function projectRootOf(cwd: string, env: NodeJS.ProcessEnv): Promise<string> {
  const result = await run(["git", "-C", cwd, "rev-parse", "--show-toplevel"], env);
  const top = result.stdout.trim();
  return result.code === 0 && top !== "" ? top : resolve(cwd);
}

export function opencodePaths(projectRoot: string, env: NodeJS.ProcessEnv): OpencodePaths {
  const globalDir = hostConfigDir(env);
  const dot = join(projectRoot, ".opencode");
  return {
    projectRoot,
    globalFiles: ["config.json", "opencode.json", "opencode.jsonc"].map((name) =>
      join(globalDir, name),
    ),
    projectFiles: [
      join(projectRoot, "opencode.json"),
      join(projectRoot, "opencode.jsonc"),
      join(dot, "opencode.json"),
      join(dot, "opencode.jsonc"),
    ],
    selection: {
      global: opencodeGlobalPluginSelectionPath(opencodeConfigRoot({ env })),
      project: opencodePluginSelectionPath(projectRoot),
    },
    tooluConfig: opencodeProjectConfigPath(projectRoot),
  };
}
