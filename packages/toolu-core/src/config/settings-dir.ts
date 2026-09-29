/**
 * `toolu_settings_dir` (#253, split out in #260): where the plugin settings
 * files are. No zod, so a hook can find its list before loading anything else.
 */
import { statSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { envValue, type HostEnv } from "../host/host-name.ts";
import { pluginRoot } from "../host/host-roots.ts";

function isDirectory(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}

/** `toolu_settings_dir`: `TOOLU_SETTINGS_DIR`, then `~/.claude/settings`, then `<plugin root>/settings`. */
export function settingsDir(
  options: { env?: HostEnv; pluginRoot?: string } = {},
): string | undefined {
  const env = options.env ?? process.env;
  const explicit = envValue(env, "TOOLU_SETTINGS_DIR");
  if (explicit !== undefined) {
    return explicit;
  }
  const legacy = join(envValue(env, "HOME") ?? homedir(), ".claude", "settings");
  if (isDirectory(legacy)) {
    return legacy;
  }
  const root = options.pluginRoot ?? pluginRoot({ env });
  return root === undefined ? undefined : join(root, "settings");
}
