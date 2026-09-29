/**
 * Where the toolu config files are, and whether either exists (#253, split out
 * in #260). Stat only, and no zod: the standalone `mcp__` hook asks this on
 * every MCP call before it decides whether to load the config at all.
 */
import { statSync } from "node:fs";
import { join } from "node:path";
import { detectHost } from "../host/host-detect.ts";
import type { HostEnv, HostName } from "../host/host-name.ts";
import { configRoot, projectConfigPath } from "../host/host-roots.ts";

export type ConfigFileOptions = { env?: HostEnv; host?: HostName; cwd?: string };

export type ConfigFiles = {
  files: { user: string; project: string | undefined };
  host: HostName;
};

export function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** `<config root>/toolu.config.json` and `<project>/<dirname>/toolu.config.json`. */
export function configFiles(options: ConfigFileOptions): ConfigFiles {
  const env = options.env ?? process.env;
  const host = options.host ?? detectHost({ env });
  const scoped = options.cwd === undefined ? { env, host } : { env, host, cwd: options.cwd };
  const files = {
    user: join(configRoot(scoped), "toolu.config.json"),
    project: projectConfigPath(scoped),
  };
  return { files, host };
}

/** Is either config file on disk? Stat only: no read, no parse. */
export function configExists(options: ConfigFileOptions = {}): boolean {
  const { files } = configFiles(options);
  return isFile(files.user) || (files.project !== undefined && isFile(files.project));
}
