/** Detect runtime host; OpenCode via override, env, or project markers (#211). */
import { existsSync } from "node:fs";
import { join } from "node:path";
import type { HostDetectOptions, TooluHost } from "./types.ts";

const OPENCODE_ENV_KEYS = [
  "OPENCODE_HOME",
  "OPENCODE_CONFIG",
  "OPENCODE_PROJECT_DIR",
  "OPENCODE_BIN",
] as const;

function hasOpencodeEnv(env: NodeJS.ProcessEnv): boolean {
  return OPENCODE_ENV_KEYS.some((key) => {
    const value = env[key];
    return typeof value === "string" && value.length > 0;
  });
}

function hasOpencodeProjectMarker(projectRoot: string): boolean {
  return existsSync(join(projectRoot, ".opencode"));
}

/** Resolve host from env overrides and optional project markers. */
export function detectHost(options: HostDetectOptions = {}): TooluHost {
  const env = options.env ?? process.env;
  const override = env.TOOLU_HOST_OVERRIDE;
  if (override === "claude" || override === "codex" || override === "opencode") {
    return override;
  }

  if (hasOpencodeEnv(env)) {
    return "opencode";
  }

  const projectRoot = options.projectRoot ?? env.TOOLU_PROJECT_DIR;
  if (projectRoot && hasOpencodeProjectMarker(projectRoot)) {
    return "opencode";
  }

  if (env.PLUGIN_ROOT) {
    return "codex";
  }

  return "claude";
}
