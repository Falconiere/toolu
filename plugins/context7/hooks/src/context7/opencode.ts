/**
 * context7 on OpenCode (#348): the startup instruction the plugin's own
 * SessionStart gives, naming the published helper the way the agent's bash
 * must run it.
 */

/** The id the OpenCode surface generator gives this plugin's `context7` skill. */
export const OPENCODE_SKILL = "context7-context7";

/** The OpenCode adapter runs every toolu hook with `TOOLU_HOST_OVERRIDE=opencode`. */
export function onOpencode(): boolean {
  return process.env.TOOLU_HOST_OVERRIDE === "opencode";
}

const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

/**
 * The published symlink runs under the hook's resolved Bun, which ignores the
 * project `.env` (Bun loads it from the working directory by default). A
 * user's own file at that path brings its own interpreter and runs alone.
 */
export function command(path: string, symlink: boolean): string {
  return symlink ? `${quote(process.execPath)} --no-env-file ${quote(path)}` : quote(path);
}

export function instruction(run: string): string {
  return `context7 (library docs) — for ANY third-party library/framework question (API usage, current docs, code examples, version behavior) you MUST run ${run} FIRST (\`search <library>\` to resolve the ID, then \`docs <id> <query>\`) BEFORE answering from memory or searching the web. Web search is a FALLBACK ONLY when context7 lacks coverage or the command exits non-zero (22 is an HTTP error such as a 429 rate limit). Syntax: skill({ name: "${OPENCODE_SKILL}" }). CONTEXT7_API_KEY is optional and read from the environment only, never from .env.`;
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
