/**
 * The committed generated surface, parsed by the pinned host in an isolated
 * profile: every catalog skill loads, every agent keeps its subagent mode and
 * default tool deny, and every command keeps a runnable template.
 * `bun run probe:opencode-surface` and the acceptance run (#362) share it.
 */
import { readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { z } from "zod";
import { readTree } from "../../../tools/toolu-opencode/scripts/lib/emit.ts";
import { debugJson } from "./host-run.ts";
import { ROOT } from "./results.ts";
import { ContractError } from "./schema.ts";
import { openSession } from "./session.ts";

const OUT_DIR = join(ROOT, "tools/toolu-opencode/generated");
const SurfaceRef = z.object({ id: z.string() });
const Catalog = z.object({
  plugins: z.array(
    z.object({
      name: z.string(),
      skills: z.array(SurfaceRef),
      agents: z.array(SurfaceRef),
      commands: z.array(SurfaceRef),
    }),
  ),
});
type CatalogPlugin = z.infer<typeof Catalog>["plugins"][number];

export type SurfaceCounts = { plugins: number; skills: number; agents: number; commands: number };

function record(value: unknown, label: string): Record<string, unknown> {
  const result = z.record(z.string(), z.unknown()).safeParse(value);
  if (!result.success) throw new ContractError(`${label} is not a record`);
  return result.data;
}

function checkAgent(agents: Record<string, unknown>, id: string): void {
  const parsed = record(agents[id], `OpenCode agent ${id}`);
  if (parsed.mode !== "subagent")
    throw new ContractError(`OpenCode agent ${id} lost subagent mode`);
  const permission = record(parsed.permission, `OpenCode agent ${id} permission`);
  if (permission["*"] !== "deny") {
    throw new ContractError(`OpenCode agent ${id} lost its default tool deny`);
  }
}

function checkCommand(commands: Record<string, unknown>, id: string): void {
  const parsed = record(commands[id], `OpenCode command ${id}`);
  if (typeof parsed.template !== "string" || !parsed.template.trim()) {
    throw new ContractError(`OpenCode command ${id} has no runnable template`);
  }
}

function checkPlugin(
  plugin: CatalogPlugin,
  loaded: {
    skills: Set<string>;
    agents: Record<string, unknown>;
    commands: Record<string, unknown>;
  },
): void {
  for (const skill of plugin.skills) {
    if (!loaded.skills.has(skill.id))
      throw new ContractError(`OpenCode did not load skill ${skill.id}`);
  }
  for (const agent of plugin.agents) checkAgent(loaded.agents, agent.id);
  for (const command of plugin.commands) checkCommand(loaded.commands, command.id);
}

/** Load the generated tree as project files on the pinned host; throws on the first surface it lost. */
export async function probeGeneratedSurface(
  bin: string,
  cacheRoot: string,
): Promise<SurfaceCounts> {
  const catalog = Catalog.parse(
    JSON.parse(readFileSync(join(OUT_DIR, "opencode.toolu.json"), "utf8")),
  );
  const files = Object.fromEntries(
    [...readTree(OUT_DIR)].map(([path, content]) => [
      `.opencode/${relative(OUT_DIR, path)}`,
      content,
    ]),
  );
  using session = openSession(cacheRoot, { files });
  const skills = z
    .array(z.looseObject({ name: z.string() }))
    .parse(await debugJson(bin, session, ["skill"]));
  const config = record(await debugJson(bin, session, ["config"]), "OpenCode config");
  const loaded = {
    skills: new Set(skills.map((skill) => skill.name)),
    agents: record(config.agent, "OpenCode agents"),
    commands: record(config.command, "OpenCode commands"),
  };
  for (const plugin of catalog.plugins) checkPlugin(plugin, loaded);
  const sum = (pick: (plugin: CatalogPlugin) => readonly unknown[]): number =>
    catalog.plugins.reduce((total, plugin) => total + pick(plugin).length, 0);
  return {
    plugins: catalog.plugins.length,
    skills: sum((plugin) => plugin.skills),
    agents: sum((plugin) => plugin.agents),
    commands: sum((plugin) => plugin.commands),
  };
}
