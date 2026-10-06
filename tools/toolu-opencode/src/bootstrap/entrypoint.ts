/**
 * A plugin's hook entries (#342, #341), read from the `hooks.json` routing
 * that Claude Code and Codex already run. An entry is either the generated Bun
 * launcher or the #412 native launcher, which still names its fallback bundle.
 * Anything else is reported instead of being skipped.
 */
import { readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { z } from "zod";

export type StartupEntry = { name: string; bundle: string; command?: string };

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
const NATIVE_RUN =
  /(?:exec )?"\$t" (?:(?<plugin>[a-z0-9]+(?:-[a-z0-9]+)*) )?hook (?<name>[a-z0-9]+(?:-[a-z0-9]+)*) --event (?<event>[A-Z][A-Za-z]+) --plugin-root "\$\{CLAUDE_PLUGIN_ROOT\}"/u;

/** The native command's identity is its binary invocation and fallback bundle. */
function nativeEntry(command: string, plugin: string, event: HookEventName): string | undefined {
  if (
    !command.startsWith('t=; if [ -n "$TOOLU_BIN" ]; then') ||
    !command.includes("--hook-protocol") ||
    !command.includes('if [ -z "$TOOLU_BIN" ]; then b=;')
  )
    return undefined;
  const invoked = NATIVE_RUN.exec(command)?.groups;
  const bundle = LAUNCHED_BUNDLE.exec(command)?.[1];
  if (
    invoked?.plugin !== (plugin === "toolu" ? undefined : plugin) ||
    invoked?.event !== event ||
    invoked?.name !== bundle
  )
    return undefined;
  return bundle;
}

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
): { ok: true; name: string; command?: string } | { ok: false; reason: string } {
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
  if (command.includes("--hook-protocol")) {
    const native = nativeEntry(command, plugin, event);
    return native === undefined ? unsupported : { ok: true, name: native, command };
  }
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
      entries.push(
        entry.command === undefined
          ? { name: entry.name, bundle }
          : { name: entry.name, bundle, command: entry.command },
      );
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
