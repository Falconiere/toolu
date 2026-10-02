/**
 * A plugin's startup entries (#342), read from the `hooks.json` routing that
 * Claude Code and Codex already run: every SessionStart hook whose matcher
 * covers `startup`, in file order. Each must be the generated Bun launcher
 * (`check:hooks-json` enforces the same form), so the entry is the committed
 * `hooks/dist/<entry>.js` bundle it names. Anything else is not a startup this
 * host can run, and says so instead of being skipped.
 */
import { readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import { z } from "zod";

export type StartupEntry = { name: string; bundle: string };

export type EntryPlan = { ok: true; entries: StartupEntry[] } | { ok: false; reason: string };

/** Keys a host adds beside these (a timeout, a status message) do not change what runs. */
const SessionStartHook = z.looseObject({ type: z.string(), command: z.string().optional() });

const SessionStartGroup = z.looseObject({
  matcher: z.string().optional(),
  hooks: z.array(SessionStartHook),
});

/** How much of an unsupported command the reason quotes. */
const COMMAND_EXCERPT = 120;

const HooksFile = z.looseObject({
  hooks: z.looseObject({ SessionStart: z.array(SessionStartGroup).optional() }),
});

const LAUNCHED_BUNDLE = /"\$\{CLAUDE_PLUGIN_ROOT\}\/hooks\/dist\/([a-z0-9-]+)\.js"/u;

/** Plugin init is a fresh `startup`; an absent, empty or `*` matcher covers every source. */
function coversStartup(matcher: string | undefined): boolean {
  if (matcher === undefined || matcher === "" || matcher === "*") return true;
  return matcher.split("|").includes("startup");
}

function readHooksFile(path: string): unknown {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined;
    throw error;
  }
}

function isFile(path: string): boolean {
  return statSync(path, { throwIfNoEntry: false })?.isFile() ?? false;
}

/** The entry a hook launches, or why it is not a launcher this host can run. */
function entryOf(
  hook: z.infer<typeof SessionStartHook>,
  plugin: string,
): { ok: true; name: string } | { ok: false; reason: string } {
  if (hook.type !== "command") {
    return { ok: false, reason: `unsupported SessionStart hook type ${JSON.stringify(hook.type)}` };
  }
  const command = hook.command ?? "";
  const name = LAUNCHED_BUNDLE.exec(command)?.[1];
  const quoted =
    command.length > COMMAND_EXCERPT ? `${command.slice(0, COMMAND_EXCERPT)}…` : command;
  const unsupported = {
    ok: false as const,
    reason: `unsupported SessionStart command ${JSON.stringify(quoted)}; regenerate it with \`bun run tooling/src/check-hooks-json.ts --print ${plugin} SessionStart <entry>\``,
  };
  if (name === undefined) return unsupported;
  try {
    const expected = launcherCommand({ plugin, event: "SessionStart", entry: name });
    return command === expected ? { ok: true, name } : unsupported;
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
}

/** Every startup entry of the plugin at `pluginDir`; no `hooks.json` means none. */
export function pluginStartupEntries(pluginDir: string): EntryPlan {
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
  const groups = parsed.data.hooks.SessionStart ?? [];
  for (const group of groups.filter((g) => coversStartup(g.matcher))) {
    for (const hook of group.hooks) {
      const entry = entryOf(hook, plugin);
      if (!entry.ok) return entry;
      const bundle = join(pluginDir, "hooks", "dist", `${entry.name}.js`);
      if (!isFile(bundle)) return { ok: false, reason: `${entry.name}: missing startup bundle` };
      entries.push({ name: entry.name, bundle });
    }
  }
  return { ok: true, entries };
}
