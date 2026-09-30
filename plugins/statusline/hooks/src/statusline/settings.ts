/**
 * Claude Code's `settings.json` `statusLine` key. A plugin cannot declare it,
 * so `/statusline:setup` writes it, pointing at the stable path the
 * SessionStart hook publishes. That path is a Bun program now: a command that
 * still runs it through `bash` (what setup wrote before the port) is legacy.
 */
import { homedir } from "node:os";
import { envValue, type HostEnv } from "@toolu/core/host";

/** Substring every command pointing at the published statusline contains. */
export const MARKER = "statusline/statusline.sh";

export function settingsPath(env: HostEnv): string {
  const dir = envValue(env, "CLAUDE_CONFIG_DIR") ?? `${envValue(env, "HOME") ?? homedir()}/.claude`;
  return `${dir}/settings.json`;
}

/** A literal `~` for the default dir (Claude Code runs the command through a shell), else the quoted explicit dir. */
export function desiredCommand(env: HostEnv): string {
  const dir = envValue(env, "CLAUDE_CONFIG_DIR");
  return dir === undefined ? `~/.claude/${MARKER}` : `"${dir}/${MARKER}"`;
}

/** A `statusLine` command that runs the published statusline through `bash`. */
export function isLegacyCommand(command: unknown): boolean {
  return typeof command === "string" && /^\s*bash\s/.test(command) && command.includes(MARKER);
}
