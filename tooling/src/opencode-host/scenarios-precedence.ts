/**
 * Live precedence scenarios (#345): a user's skill, agent, command and
 * permission rule win over toolu's contributions, and a user skill in any root
 * the pinned host scans leaves exactly one copy.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { z } from "zod";
import {
  SELECTION,
  countOf,
  editConfig,
  generatedRoot,
  install,
  lines,
  passes,
  selection,
  services,
  skillAndRead,
  skillFile,
  skills,
  type Observed,
  type Scripts,
} from "./install-host.ts";
import type { EntryContext, EntryResult } from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

const USER_DEBUG = ".opencode/skills/toolu-debug/SKILL.md";
const USER_AGENT = ".opencode/agents/toolu-implementer.md";

function precedenceFiles(): Record<string, string> {
  return {
    [SELECTION]: selection(["toolu"]),
    [USER_DEBUG]: skillFile("toolu-debug", "User debug"),
    ".opencode/skills/my-skill/SKILL.md": skillFile("my-skill", "Mine"),
    [USER_AGENT]: "---\ndescription: User implementer\n---\nUser prompt\n",
  };
}

function precedenceConfig(): Record<string, unknown> {
  return {
    agent: { "toolu-quick-task": { description: "User quick" } },
    command: { "toolu-commit-1e9b92d5": { template: "user template" } },
  };
}

/** The resolved agent's fields the scenario asserts. */
function agentFacts(agents: Array<Record<string, unknown>>, name: string) {
  const agent = agents.find((a) => a.name === name) ?? {};
  const rules = z
    .array(z.looseObject({ permission: z.string(), action: z.string() }))
    .safeParse(agent.permission);
  return {
    description: String(agent.description),
    prompt: String(agent.prompt),
    mode: String(agent.mode),
    denyAll: rules.success && rules.data.some((r) => r.permission === "*" && r.action === "deny"),
  };
}

export async function precedence(ctx: EntryContext): Promise<EntryResult> {
  const files = precedenceFiles();
  const scripts: Scripts = {};
  using s = install(ctx, files, scripts, precedenceConfig);
  const { rows, log } = await skills(ctx, s);
  const listed = await services(ctx, s);
  const quick = agentFacts(listed.agents, "toolu-quick-task");
  const implementer = agentFacts(listed.agents, "toolu-implementer");
  const commit = listed.commands.find((c) => c.name === "toolu-commit-1e9b92d5");
  const generated = generatedRoot(rows, "toolu-commit-e11d9d00");
  editConfig(s, (config) => Object.assign(config, { permission: { external_directory: "ask" } }));
  const asked = await skillAndRead(ctx, s, scripts, "surfaces.precedence", generated);
  const observed = {
    debugOnce: countOf(rows, "toolu-debug"),
    debugIsUsers:
      rows.find((r) => r.name === "toolu-debug")?.location === join(s.sb.project, USER_DEBUG),
    mySkill: countOf(rows, "my-skill"),
    quickDescription: quick.description,
    quickPrompt: quick.prompt.startsWith("## Instructions"),
    implementerPrompt: implementer.prompt,
    implementerMode: implementer.mode,
    implementerDenyAll: implementer.denyAll,
    commitTemplate: String(commit?.template),
    filesUnchanged: Object.entries(files).every(([rel, text]) => s.sb.read(rel) === text),
    keptNoted: lines(log, "skill toolu-debug kept from") > 0,
    mergedNoted: lines(log, "agent toolu-quick-task merged under your config") > 0,
    userRuleDecides: asked.read !== "completed" && asked.skill === "completed",
  };
  const expected = {
    debugOnce: 1,
    debugIsUsers: true,
    mySkill: 1,
    quickDescription: "User quick",
    quickPrompt: true,
    implementerPrompt: "User prompt",
    implementerMode: "subagent",
    implementerDenyAll: true,
    commitTemplate: "user template",
    filesUnchanged: true,
    keptNoted: true,
    mergedNoted: true,
    userRuleDecides: true,
  };
  return { pass: passes(observed, expected), observed };
}

type RootCase = {
  label: string;
  place: (s: ProbeSession) => string | undefined;
  env?: Record<string, string>;
  cwd?: (s: ProbeSession) => string;
};

function placeAt(dir: string, text = skillFile("toolu-debug", "User copy")): string {
  const path = join(dir, "toolu-debug/SKILL.md");
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  return path;
}

const ROOT_CASES: RootCase[] = [
  { label: "home-claude", place: (s) => placeAt(join(s.sb.home, ".claude/skills")) },
  { label: "home-agents", place: (s) => placeAt(join(s.sb.home, ".agents/skills")) },
  {
    label: "project-claude-upwalk",
    place: (s) => placeAt(join(s.sb.project, ".claude/skills")),
    cwd: (s) => join(s.sb.project, "packages/app"),
  },
  { label: "config-dir", place: (s) => placeAt(join(s.sb.root, "custom/skills")) },
  { label: "home-opencode", place: (s) => placeAt(join(s.sb.home, ".opencode/skills")) },
  { label: "skills-paths", place: (s) => placeAt(join(s.sb.root, "manual")) },
  {
    label: "external-disabled",
    place: (s) => (placeAt(join(s.sb.home, ".claude/skills")), undefined),
    env: { OPENCODE_DISABLE_EXTERNAL_SKILLS: "1" },
  },
  {
    label: "dot-dir",
    place: (s) => (placeAt(join(s.sb.project, ".opencode/skills/.hidden")), undefined),
  },
  {
    label: "bom-crlf",
    place: (s) =>
      placeAt(
        join(s.sb.project, ".opencode/skills"),
        "﻿---\r\nname: toolu-debug\r\ndescription: User copy\r\n---\r\nbody\r\n",
      ),
  },
];

/** Run one root case; `undefined` placement means toolu's copy is the one expected. */
async function rootCase(ctx: EntryContext, c: RootCase): Promise<string> {
  using s = install(ctx, { [SELECTION]: selection(["toolu"]) }, {}, (root) => ({
    skills: { paths: [join(root, "manual")] },
  }));
  Object.assign(s.env, { OPENCODE_CONFIG_DIR: join(s.sb.root, "custom") }, c.env ?? {});
  const userCopy = c.place(s);
  const cwd = c.cwd?.(s);
  if (cwd !== undefined) mkdirSync(cwd, { recursive: true });
  const { rows } = await skills(ctx, s, cwd);
  const found = rows.filter((r) => r.name === "toolu-debug");
  const location = found[0]?.location ?? "";
  const tooluCopy = location.endsWith("/generated/skills/toolu-debug/SKILL.md");
  const ok = found.length === 1 && (userCopy === undefined ? tooluCopy : location === userCopy);
  return ok ? "ok" : `${found.length}@${location}`;
}

/** Cases run one at a time: they share the host's caches and must not race. */
async function rootCases(
  ctx: EntryContext,
  remaining: readonly RootCase[],
  observed: Observed,
): Promise<Observed> {
  const [next, ...rest] = remaining;
  if (next === undefined) return observed;
  return rootCases(ctx, rest, { ...observed, [next.label]: await rootCase(ctx, next) });
}

export async function skillRoots(ctx: EntryContext): Promise<EntryResult> {
  const observed = await rootCases(ctx, ROOT_CASES, {});
  return { pass: Object.values(observed).every((v) => v === "ok"), observed };
}
