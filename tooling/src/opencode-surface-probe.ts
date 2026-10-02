#!/usr/bin/env bun
/** Parse the committed generated surface on the pinned, isolated OpenCode host. */
import { readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { z } from "zod";
import { readTree } from "../../tools/toolu-opencode/scripts/lib/emit.ts";
import { debugJson } from "./opencode-host/host-run.ts";
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { PinSchema } from "./opencode-host/schema.ts";
import { openSession } from "./opencode-host/session.ts";

const root = join(import.meta.dir, "../..");
const outDir = join(root, "tools/toolu-opencode/generated");
const pin = PinSchema.parse(
  JSON.parse(readFileSync(join(root, "tools/toolu-opencode/contract/pin.json"), "utf8")),
);
const SurfaceRef = z.object({ id: z.string() });
const catalog = z
  .object({
    plugins: z.array(
      z.object({
        name: z.string(),
        skills: z.array(SurfaceRef),
        agents: z.array(SurfaceRef),
        commands: z.array(SurfaceRef),
      }),
    ),
  })
  .parse(JSON.parse(readFileSync(join(outDir, "opencode.toolu.json"), "utf8")));

function record(value: unknown, label: string): Record<string, unknown> {
  const result = z.record(z.string(), z.unknown()).safeParse(value);
  if (!result.success) throw new Error(`${label} is not a record`);
  return result.data;
}

async function main(): Promise<void> {
  const host = await resolveHostBinary(pin);
  const files = Object.fromEntries(
    [...readTree(outDir)].map(([path, content]) => [
      `.opencode/${relative(outDir, path)}`,
      content,
    ]),
  );
  using session = openSession(hostCacheDir(pin), { files });
  const skills = z
    .array(z.looseObject({ name: z.string() }))
    .parse(await debugJson(host.bin, session, ["skill"]));
  const config = record(await debugJson(host.bin, session, ["config"]), "OpenCode config");
  const agents = record(config.agent, "OpenCode agents");
  const commands = record(config.command, "OpenCode commands");
  const skillNames = new Set(skills.map((skill) => skill.name));
  let skillCount = 0;
  let agentCount = 0;
  let commandCount = 0;
  for (const plugin of catalog.plugins) {
    for (const skill of plugin.skills) {
      if (!skillNames.has(skill.id)) throw new Error(`OpenCode did not load skill ${skill.id}`);
      skillCount += 1;
    }
    for (const agent of plugin.agents) {
      const parsed = record(agents[agent.id], `OpenCode agent ${agent.id}`);
      if (parsed.mode !== "subagent")
        throw new Error(`OpenCode agent ${agent.id} lost subagent mode`);
      const permission = record(parsed.permission, `OpenCode agent ${agent.id} permission`);
      if (permission["*"] !== "deny") {
        throw new Error(`OpenCode agent ${agent.id} lost its default tool deny`);
      }
      agentCount += 1;
    }
    for (const command of plugin.commands) {
      const parsed = record(commands[command.id], `OpenCode command ${command.id}`);
      if (typeof parsed.template !== "string" || !parsed.template.trim()) {
        throw new Error(`OpenCode command ${command.id} has no runnable template`);
      }
      commandCount += 1;
    }
  }
  process.stdout.write(
    `opencode-surface-probe: ${host.version}, ${catalog.plugins.length} plugins, ${skillCount} skills, ${agentCount} agents, ${commandCount} commands loaded\n`,
  );
}

await main();
