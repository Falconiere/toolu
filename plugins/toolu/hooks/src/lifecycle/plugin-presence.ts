/**
 * `detect_plugin_installed` as session-start.sh saw it (#263): Codex answers
 * from its SessionStart snapshot, every other host from Claude Code's
 * installed_plugins.json. `unknown` is the bash exit 2 (indeterminate).
 */
import type { HostName } from "@toolu/core/host";
import { pluginPresence, type PluginPresence } from "@toolu/core/registry";

type Env = Record<string, string | undefined>;

export function presence(spec: string, env: Env, host: HostName): PluginPresence {
  return pluginPresence(spec, { env, host: host === "codex" ? "codex" : "claude" });
}

/** `toolu_plugin_active`: installed, or indeterminate (fail open). */
export function pluginActive(spec: string, env: Env, host: HostName): boolean {
  return presence(spec, env, host) !== "absent";
}

/** `toolu_plugin_install_command`: Codex's CLI, else Claude Code's slash command. */
export function installCommand(spec: string, host: HostName): string {
  return host === "codex" ? `codex plugin add ${spec}` : `/plugin install ${spec}`;
}
