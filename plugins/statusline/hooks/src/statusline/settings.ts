/**
 * Claude Code's `settings.json` `statusLine` key. A plugin cannot declare it,
 * so `/statusline:setup` writes it, pointing at the stable path the
 * SessionStart hook publishes. That path is a Bun program now: a command that
 * still runs it through a shell (setup wrote `bash …` before the port) is legacy.
 */
import { homedir } from "node:os";
import { envValue, type HostEnv } from "@toolu/core/host";

/**
 * Substring every command pointing at the published statusline contains. The
 * published link keeps its `.sh` name while it points at `hooks/dist/statusline.js`,
 * so every `settings.json` wired before the port still names a live path.
 */
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

/**
 * A `statusLine` command that hands the published statusline to a shell as a
 * script: `bash`, `/bin/sh`, `/usr/bin/env zsh`… A `-c` command string runs it
 * as a program, which still works, so it is not legacy.
 */
export function isLegacyCommand(command: unknown): boolean {
  if (typeof command !== "string") return false;
  const words = command.trim().split(/\s+/);
  const shell = /(?:^|\/)env$/.test(words[0] ?? "") ? 1 : 0;
  const marker = words.findIndex((word) => word.includes(MARKER));
  return (
    /^(?:\S*\/)?(?:ba|z)?sh$/.test(words[shell] ?? "") &&
    marker > shell &&
    !words.slice(shell + 1, marker + 1).some((word) => /^-[a-zA-Z]*c/.test(word))
  );
}
