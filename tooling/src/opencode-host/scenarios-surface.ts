/**
 * Surface scenarios (#335): which project skills, agents and commands the
 * pinned host discovers, whether it enforces the documented skill naming
 * rule, and whether a plugin's `config` hook can inject surfaces.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { debugJson } from "./host-run.ts";
import {
  entries,
  precondition,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import { openSession, PROBE_PLUGIN, type ProbeSession } from "./session.ts";

const SkillList = z.array(z.looseObject({ name: z.string() }));
const ResolvedConfig = z.looseObject({
  command: z.record(z.string(), z.unknown()).optional(),
  agent: z.record(z.string(), z.unknown()).optional(),
  instructions: z.array(z.string()).optional(),
});

function skill(name: string): string {
  return `---\nname: ${name}\ndescription: Probe skill ${name}.\n---\nBody\n`;
}

async function skillNames(ctx: ScenarioContext, session: ProbeSession): Promise<string[]> {
  return SkillList.parse(await debugJson(ctx.bin, session, ["skill"])).map((s) => s.name);
}

async function resolvedConfig(
  ctx: ScenarioContext,
  session: ProbeSession,
): Promise<z.infer<typeof ResolvedConfig>> {
  return ResolvedConfig.parse(await debugJson(ctx.bin, session, ["config"]));
}

async function files(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    files: {
      ".opencode/skills/toolu-probe-skill/SKILL.md": skill("toolu-probe-skill"),
      ".opencode/agents/toolu-probe-agent.md":
        "---\ndescription: Probe agent.\nmode: subagent\n---\nYou are a probe.\n",
      ".opencode/commands/toolu-probe.md":
        "---\ndescription: Probe command.\n---\nprobe $ARGUMENTS\n",
    },
  });
  const config = await resolvedConfig(ctx, session);
  const observed = {
    skillDiscovered: (await skillNames(ctx, session)).includes("toolu-probe-skill"),
    agentDiscovered: Object.keys(config.agent ?? {}).includes("toolu-probe-agent"),
    commandDiscovered: Object.keys(config.command ?? {}).includes("toolu-probe"),
  };
  return verdict(Object.values(observed).every(Boolean), observed);
}

async function names(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    files: {
      ".opencode/skills/toolu-valid/SKILL.md": skill("toolu-valid"),
      ".opencode/skills/toolu--double/SKILL.md": skill("toolu--double"),
      ".opencode/skills/Bad_Name/SKILL.md": skill("Bad_Name"),
    },
  });
  const listed = await skillNames(ctx, session);
  precondition(
    "surface.names",
    listed.includes("toolu-valid"),
    "the valid skill was not discovered",
  );
  const doubleHyphenLoaded = listed.includes("toolu--double");
  const invalidCharsLoaded = listed.includes("Bad_Name");
  return verdict(!doubleHyphenLoaded && !invalidCharsLoaded, {
    doubleHyphenLoaded,
    invalidCharsLoaded,
  });
}

function injectedSkills(root: string): string {
  const dir = join(root, "injected-skills");
  mkdirSync(join(dir, "toolu-injected"), { recursive: true });
  writeFileSync(join(dir, "toolu-injected/SKILL.md"), skill("toolu-injected"));
  writeFileSync(join(dir, "README.md"), "Injected instructions.\n");
  return dir;
}

async function configHook(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [PROBE_PLUGIN],
    probeConfig: (root) => ({ configSkillsPath: injectedSkills(root) }),
  });
  const config = await resolvedConfig(ctx, session);
  precondition(
    "surface.config-hook",
    entries(session, "load").length > 0,
    "the probe plugin never loaded",
  );
  const observed = {
    commandInjected: Object.keys(config.command ?? {}).includes("toolu-probe-cmd"),
    agentInjected: Object.keys(config.agent ?? {}).includes("toolu-probe-agent"),
    instructionsInjected: (config.instructions ?? []).some((path) =>
      path.endsWith("injected-skills/README.md"),
    ),
    skillInjected: (await skillNames(ctx, session)).includes("toolu-injected"),
  };
  return verdict(Object.values(observed).every(Boolean), observed);
}

export const SURFACE_SCENARIOS: Scenario[] = [
  {
    id: "surface.files",
    axis: "surfaces",
    kind: "surface",
    mechanism: ".opencode/{skills,agents,commands}/",
    claim: "Project skills, agents and commands under .opencode/ are discovered",
    run: files,
  },
  {
    id: "surface.names",
    axis: "surfaces",
    kind: "surface",
    mechanism: "skill name validation",
    claim: "The host rejects skill names that violate the documented naming rule",
    run: names,
  },
  {
    id: "surface.config-hook",
    axis: "surfaces",
    kind: "config",
    mechanism: "config hook",
    claim: "A plugin config hook can inject commands, agents, skill paths and instructions",
    run: configHook,
  },
];
