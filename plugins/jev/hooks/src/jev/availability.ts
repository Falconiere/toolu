import { lstatSync } from "node:fs";

/** The id the OpenCode surface generator gives this plugin's `jev` skill. */
export const OPENCODE_SKILL = "jev-jev";

/** The OpenCode adapter runs every toolu hook with `TOOLU_HOST_OVERRIDE=opencode`. */
export function onOpencode(): boolean {
  return process.env.TOOLU_HOST_OVERRIDE === "opencode";
}

/** The hook already runs under the launcher's resolved Bun; PATH is not required. */
export function invocation(wrapper: string): string {
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
  // Preserved user executables supply their own shebang/interpreter, whether shell
  // or JavaScript. Forcing Bun here would break existing shell-script overrides.
  if (!lstatSync(wrapper).isSymbolicLink()) return quote(wrapper);
  // Bun loads the working directory's .env by default; the key must come from the environment.
  const flags = onOpencode() ? " --no-env-file" : "";
  return `${quote(process.execPath)}${flags} ${quote(wrapper)}`;
}

/** Where the agent finds the CLI syntax: the native skill on OpenCode, else the plugin's file. */
export function skillReference(plugin: string): string {
  return onOpencode() ? `skill({ name: "${OPENCODE_SKILL}" })` : `${plugin}/skills/jev/SKILL.md`;
}

/** Hooks and agent commands can receive different environments. Never expose the key. */
export function credentialNotice(): string {
  return process.env.TYPESAFE_API_KEY
    ? ""
    : "The Jev hook did not receive TYPESAFE_API_KEY. Before reporting Jev unavailable, check whether TYPESAFE_API_KEY is set in the command environment without printing its value; hook and command environments can differ. If absent there too, state the limitation once per task and use an explicit evidence fallback. Never invent a Jev result or read credentials from .env. ";
}
