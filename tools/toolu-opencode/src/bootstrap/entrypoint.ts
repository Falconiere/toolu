/**
 * A plugin's hook entries (#342, #341), read from the `hooks.json` routing
 * that Claude Code and Codex already run. Each must be the generated Bun
 * launcher (`check:hooks-json` enforces the same form), so the entry is the
 * committed `hooks/dist/<entry>.js` bundle it names. Anything else is not a
 * hook this host can run, and says so instead of being skipped.
 */
import { readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { z } from "zod";

export type StartupEntry = { name: string; bundle: string };

export type EntryPlan = { ok: true; entries: StartupEntry[] } | { ok: false; reason: string };

export type HookEventName = "SessionStart" | "UserPromptSubmit" | "PreCompact";

/** Keys a host adds beside these (a timeout, a status message) do not change what runs. */
const SessionStartHook = z.looseObject({ type: z.string(), command: z.string().optional() });

const SessionStartGroup = z.looseObject({
  matcher: z.string().optional(),
  hooks: z.array(SessionStartHook),
});

/** How much of an unsupported command the reason quotes. */
const COMMAND_EXCERPT = 120;

const HooksFile = z.looseObject({
  hooks: z.looseObject({
    SessionStart: z.array(SessionStartGroup).optional(),
    UserPromptSubmit: z.array(SessionStartGroup).optional(),
    PreCompact: z.array(SessionStartGroup).optional(),
  }),
});

const LAUNCHED_BUNDLE = /"\$\{CLAUDE_PLUGIN_ROOT\}\/hooks\/dist\/([a-z0-9-]+)\.js"/u;

/** An absent, empty or `*` matcher covers every token; otherwise the token must be listed. */
export function matcherCovers(matcher: string | undefined, token: string): boolean {
  if (matcher === undefined || matcher === "" || matcher === "*") return true;
  return matcher.split("|").includes(token);
}

function readHooksFile(path: string): unknown {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined;
    throw error;
  }
}

/** A bundle that cannot even be stat'ed (EACCES, ENOTDIR) is as missing as an absent one. */
function isFile(path: string): boolean {
  try {
    return statSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
  } catch {
    return false;
  }
}

/** The entry a hook launches, or why it is not a launcher this host can run. */
function entryOf(
  hook: z.infer<typeof SessionStartHook>,
  plugin: string,
  event: HookEventName,
): { ok: true; name: string } | { ok: false; reason: string } {
  if (hook.type !== "command") {
    return { ok: false, reason: `unsupported ${event} hook type ${JSON.stringify(hook.type)}` };
  }
  const command = hook.command ?? "";
  const name = LAUNCHED_BUNDLE.exec(command)?.[1];
  const quoted =
    command.length > COMMAND_EXCERPT ? `${command.slice(0, COMMAND_EXCERPT)}…` : command;
  const unsupported = {
    ok: false as const,
    reason: `unsupported ${event} command ${JSON.stringify(quoted)}; regenerate it with \`bun run tooling/src/check-hooks-json.ts --print ${plugin} ${event} <entry>\``,
  };
  if (name === undefined) return unsupported;
  try {
    const expected = launcherCommand({ plugin, event, entry: name });
    return command === expected ? { ok: true, name } : unsupported;
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
}

/** Every `event` entry whose matcher `covers` accepts; no `hooks.json` means none. */
export function pluginHookEntries(
  pluginDir: string,
  event: HookEventName,
  covers: (matcher: string | undefined) => boolean,
): EntryPlan {
  const path = join(pluginDir, "hooks", "hooks.json");
  let raw: unknown;
  try {
    raw = readHooksFile(path);
  } catch (error) {
    return { ok: false, reason: `invalid hooks.json: ${String(error)}` };
  }
  if (raw === undefined) return { ok: true, entries: [] };
  const parsed = HooksFile.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, reason: `invalid hooks.json: ${z.prettifyError(parsed.error)}` };
  }
  const plugin = basename(pluginDir);
  const entries: StartupEntry[] = [];
  const groups = parsed.data.hooks[event] ?? [];
  for (const group of groups.filter((g) => covers(g.matcher))) {
    for (const hook of group.hooks) {
      const entry = entryOf(hook, plugin, event);
      if (!entry.ok) return entry;
      const bundle = join(pluginDir, "hooks", "dist", `${entry.name}.js`);
      const missing =
        event === "SessionStart" ? "missing startup bundle" : `missing ${event} bundle`;
      if (!isFile(bundle)) return { ok: false, reason: `${entry.name}: ${missing}` };
      entries.push({ name: entry.name, bundle });
    }
  }
  return { ok: true, entries };
}

/** Every startup entry of the plugin at `pluginDir`; no `hooks.json` means none. */
export function pluginStartupEntries(pluginDir: string): EntryPlan {
  return pluginHookEntries(pluginDir, "SessionStart", (matcher) =>
    matcherCovers(matcher, "startup"),
  );
}
