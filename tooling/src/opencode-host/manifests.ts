/**
 * What each catalog plugin needs from a host, derived from its own Claude Code
 * manifests (#335): hook events and matchers in `hooks/hooks.json`, registry
 * modules in `hooks/src/register.ts`, and skill/command/agent files. The
 * capability matrix must cover at least these axes, so a new hook or plugin
 * cannot slip past the OpenCode contract.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import type { MatrixAxis } from "./schema.ts";

type PluginSurfaces = { skills: number; commands: number; agents: number };

const HooksJson = z.looseObject({
  hooks: z.record(z.string(), z.array(z.looseObject({ matcher: z.string().optional() }))),
});

const EVENT_AXES: Record<string, MatrixAxis> = {
  SessionStart: "startup",
  UserPromptSubmit: "prompt",
  PreCompact: "compaction",
  PostToolUse: "postTool",
};

const PRE_TOOL_AXES: Array<[RegExp, MatrixAxis]> = [
  [/Bash|Shell|Edit|Write|MultiEdit|apply_patch|Grep/, "tools"],
  [/mcp__/, "mcp"],
  [/Agent|Task|spawn_agent/, "task"],
];

/** Plugin directories that carry a Claude Code manifest. */
export function catalogPlugins(pluginsDir: string): string[] {
  return readdirSync(pluginsDir, { withFileTypes: true })
    .filter(
      (entry) =>
        entry.isDirectory() &&
        existsSync(join(pluginsDir, entry.name, ".claude-plugin/plugin.json")),
    )
    .map((entry) => entry.name)
    .toSorted();
}

function hookAxes(pluginDir: string): MatrixAxis[] {
  const path = join(pluginDir, "hooks/hooks.json");
  if (!existsSync(path)) return [];
  const { hooks } = HooksJson.parse(JSON.parse(readFileSync(path, "utf8")));
  return Object.entries(hooks).flatMap(([event, groups]) => {
    const direct = EVENT_AXES[event];
    if (direct !== undefined) return [direct];
    if (event !== "PreToolUse") return [];
    return groups.flatMap((group) =>
      PRE_TOOL_AXES.filter(([pattern]) => pattern.test(group.matcher ?? "")).map(
        ([, axis]) => axis,
      ),
    );
  });
}

function registryAxes(pluginDir: string): MatrixAxis[] {
  const path = join(pluginDir, "hooks/src/register.ts");
  if (!existsSync(path)) return [];
  const source = readFileSync(path, "utf8");
  const axes: MatrixAxis[] = [];
  if (/event:\s*"tool\/pre"/.test(source)) axes.push("tools");
  if (/event:\s*"tool\/post"/.test(source)) axes.push("postTool");
  return axes;
}

/** Axes a plugin's manifests prove it needs. */
export function derivedAxes(pluginDir: string): Set<MatrixAxis> {
  return new Set([...hookAxes(pluginDir), ...registryAxes(pluginDir)]);
}

function countFiles(dir: string, pick: (path: string) => boolean): number {
  if (!existsSync(dir)) return 0;
  return readdirSync(dir).filter((name) => pick(join(dir, name))).length;
}

export function pluginSurfaces(pluginDir: string): PluginSurfaces {
  return {
    skills: countFiles(join(pluginDir, "skills"), (path) => existsSync(join(path, "SKILL.md"))),
    commands: countFiles(join(pluginDir, "commands"), (path) => path.endsWith(".md")),
    agents: countFiles(join(pluginDir, "agents"), (path) => path.endsWith(".md")),
  };
}
