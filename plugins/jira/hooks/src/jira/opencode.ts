/**
 * jira on OpenCode (#351): the startup instruction the plugin's own
 * SessionStart gives, naming the published helper the way the agent's bash
 * must run it, and the read/write boundary the skill keeps.
 */
import type { Env } from "./creds.ts";

/** The id the OpenCode surface generator gives this plugin's `jira` skill. */
export const OPENCODE_SKILL = "jira-jira";

/** The OpenCode adapter runs every toolu hook, and `shell.env` every bash call, with this override. */
export function onOpencode(env: Env = process.env): boolean {
  return env["TOOLU_HOST_OVERRIDE"] === "opencode";
}

const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

/**
 * The published symlink runs under the hook's resolved Bun, which then ignores
 * the project `.env` (Bun loads it from the working directory by default). A
 * user's own file at that path brings its own interpreter and runs alone.
 */
export function command(path: string, symlink: boolean): string {
  return symlink ? `${quote(process.execPath)} --no-env-file ${quote(path)}` : quote(path);
}

export function instruction(run: string): string {
  return `jira (issue tracker) — when the user mentions Jira, a JQL query, an issue key like ABC-123 or an atlassian.net/browse link, run ${run} <family> <action> [options] (syntax: skill({ name: "${OPENCODE_SKILL}" })) instead of the Atlassian MCP. Read-only calls (search, issue get, board/sprint/project/user lookups) may run directly. Never create, update, comment on, transition, assign or delete an issue, or change a sprint, worklog or attachment, unless the user asked for that change; plan it first as the skill describes. Credentials come from JIRA_* in the environment or the jira CLI login, never from .env; without them the command prints setup help and exits 1.`;
}

/** SessionStart stdin names its trigger; unreadable stdin counts as a plain start. */
export async function compacting(): Promise<boolean> {
  let input: unknown;
  try {
    input = JSON.parse(await Bun.stdin.text());
  } catch {
    return false;
  }
  return (
    input !== null && typeof input === "object" && "source" in input && input.source === "compact"
  );
}

/**
 * The environment of a plan's probe and checks. Their "$JIRA" is the
 * `#!/usr/bin/env bun` symlink, so on OpenCode `BUN_OPTIONS` carries
 * `--no-env-file` to keep the project `.env` out one level down too.
 */
export function nestedEnv(env: Env): Env {
  if (!onOpencode(env)) return env;
  const options = env["BUN_OPTIONS"] ?? "";
  return { ...env, BUN_OPTIONS: options === "" ? "--no-env-file" : `${options} --no-env-file` };
}
