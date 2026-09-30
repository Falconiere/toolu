/** Discover SessionStart / register entrypoints per plugin (#211, #269, #265).
 * A register entrypoint wins over a session-start one; within each, the
 * committed TypeScript bundle (hooks/dist/<entry>.js) wins over the bash
 * script it replaced. Missing all four → null (caller skips; not every
 * plugin registers).
 */
import { existsSync } from "node:fs";
import { basename, join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";

export function pluginBootstrapScript(pluginDir: string): string | null {
  const candidates = [
    join(pluginDir, "hooks", "dist", "register.js"),
    join(pluginDir, "hooks", "register.sh"),
    join(pluginDir, "hooks", "dist", "session-start.js"),
    join(pluginDir, "hooks", "session-start.sh"),
  ];
  return candidates.find((path) => existsSync(path)) ?? null;
}

export type BootstrapCommand = { argv: string[]; env: Record<string, string> };

/**
 * How to run `script`: bash for a shell entrypoint; for a bundle, the plugin's
 * generated hooks.json launcher, so Bun resolves exactly as on every other
 * host (TOOLU_BUN, PATH, ~/.bun/bin/bun) and a missing runtime degrades to
 * the launcher's advisory instead of failing the bootstrap.
 */
export function bootstrapCommand(
  script: string,
  plugin: string,
  pluginDir: string,
): BootstrapCommand {
  if (!script.endsWith(".js")) return { argv: ["bash", script], env: {} };
  const entry = basename(script, ".js");
  const command = launcherCommand({ plugin, event: "SessionStart", entry });
  return { argv: ["sh", "-c", command], env: { CLAUDE_PLUGIN_ROOT: pluginDir } };
}
