/**
 * Live install and discovery scenarios (#345) on the pinned host.
 *
 * The packed `@toolu/opencode` loads through the npm route in an isolated
 * profile. Its `config` hook contributes the selected plugins' generated
 * skills, agents and commands, and these scenarios read what the host itself
 * discovered: `opencode debug skill`, the server's `/agent` and `/command`
 * lists, `debug config`, the native `skill` and `read` tools in a scripted
 * session, and the host log (`--print-logs`). Nothing is written into the
 * project's discovery directories, and user definitions keep precedence.
 */
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { z } from "zod";
import { runHost } from "./host-run.ts";
import {
  SELECTION,
  catalogIds,
  countOf,
  editConfig,
  generatedRoot,
  host,
  install,
  lines,
  named,
  noSurfaceFiles,
  passes,
  selection,
  services,
  skillAndRead,
  skillFile,
  skills,
  toolResults,
  type Scripts,
} from "./install-host.ts";
import {
  ROOT,
  installShim,
  npmSpec,
  packTarball,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { precedence, skillRoots } from "./scenarios-precedence.ts";

const PR_BABYSIT_SKILLS = [
  "pr-babysit-babysit-73c340c6",
  "toolu-commit-e11d9d00",
  "toolu-debug",
  "toolu-deep-research",
  "toolu-orchestrator",
  "toolu-review-and-commit-1a591621",
  "toolu-setup",
];
const PR_BABYSIT_COMMANDS = [
  "pr-babysit-babysit-ff6e5a3d",
  "toolu-commit-1e9b92d5",
  "toolu-review-and-commit-db159d0c",
];

async function npmClean(ctx: EntryContext): Promise<EntryResult> {
  const ids = catalogIds();
  const scripts: Scripts = {};
  using s = install(ctx, { [SELECTION]: selection(["pr-babysit"]) }, scripts);
  const { rows } = await skills(ctx, s);
  const listed = await services(ctx, s);
  const commands = listed.commands.filter((c) => c.source === "command");
  const generated = generatedRoot(rows, "toolu-commit-e11d9d00");
  const run1 = await skillAndRead(ctx, s, scripts, "surfaces.npm-clean", generated);
  const observed = {
    skills: named(rows, ids.skills).toSorted().join(","),
    located: rows
      .filter((r) => ids.skills.has(r.name))
      .every((r) => r.location === join(generated, "skills", r.name, "SKILL.md")),
    agents: named(
      listed.agents.map((a) => ({ name: String(a.name) })),
      ids.agents,
    ).length,
    commands: named(
      commands.map((c) => ({ name: String(c.name) })),
      ids.commands,
    ).join(","),
    skillTool: run1.skill,
    skillBody: run1.skillText.includes("Base directory for this skill"),
    described: run1.described,
    sharedRead: run1.read,
    noSurfaceFiles: noSurfaceFiles(s),
  };
  const expected = {
    skills: PR_BABYSIT_SKILLS.join(","),
    located: true,
    agents: 5,
    commands: PR_BABYSIT_COMMANDS.join(","),
    skillTool: "completed",
    skillBody: true,
    described: true,
    sharedRead: "completed",
    noSurfaceFiles: true,
  };
  return { pass: passes(observed, expected), observed };
}

const UPDATED = "Updated toolu-debug description for surfaces.lifecycle";

/** A second release whose toolu-debug skill has a different description. */
function updatedTarball(): string {
  const work = mkdtempSync(join(tmpdir(), "toolu-surfaces-update-"));
  return packTarball(work, (stage) => {
    const path = join(stage, "generated/skills/toolu-debug/SKILL.md");
    const text = readFileSync(path, "utf8").replace(
      /^description: .*$/m,
      `description: "${UPDATED}"`,
    );
    writeFileSync(path, text);
  });
}

async function lifecycle(ctx: EntryContext): Promise<EntryResult> {
  const ids = catalogIds();
  const mine = ".opencode/skills/my-skill/SKILL.md";
  const mineText = skillFile("my-skill", "Mine");
  using s = install(ctx, { [SELECTION]: selection(["pr-babysit"]), [mine]: mineText });
  const first = (await skills(ctx, s)).rows;
  s.sb.write(SELECTION, selection(["jev"]));
  const reselected = (await skills(ctx, s)).rows;
  const tarball = updatedTarball();
  s.sb.write(SELECTION, selection(["toolu"]));
  editConfig(s, (config) => Object.assign(config, { plugin: [npmSpec(tarball)] }));
  const updated = (await skills(ctx, s)).rows.find((r) => r.name === "toolu-debug");
  editConfig(s, (config) => Object.assign(config, { plugin: [] }));
  const removed = (await skills(ctx, s)).rows;
  const removedConfig = z
    .looseObject({ agent: z.record(z.string(), z.unknown()).optional() })
    .parse(JSON.parse((await host(ctx, s, ["debug", "config"])).stdout));
  const observed = {
    first: named(first, ids.skills).length,
    reselected: named(reselected, ids.skills).join(","),
    mineKept: countOf(reselected, "my-skill") === 1 && s.sb.read(mine) === mineText,
    updated: updated?.description === UPDATED,
    removedSkills: named(removed, ids.skills).length,
    removedAgents: Object.keys(removedConfig.agent ?? {}).filter((a) => ids.agents.has(a)).length,
    noSurfaceFiles: noSurfaceFiles(s, ["my-skill"]),
  };
  const expected = {
    first: 7,
    reselected: "jev-jev",
    mineKept: true,
    updated: true,
    removedSkills: 0,
    removedAgents: 0,
    noSurfaceFiles: true,
  };
  return { pass: passes(observed, expected), observed };
}

async function bothRoutes(ctx: EntryContext): Promise<EntryResult> {
  const ids = catalogIds();
  using s = install(ctx, { [SELECTION]: selection(["toolu"]) });
  installShim(s);
  s.env.TOOLU_REPO_ROOT = ROOT;
  const { rows, log } = await skills(ctx, s);
  const toolu = rows.filter((r) => ids.skills.has(r.name));
  const observed = {
    skills: toolu.length,
    distinct: new Set(toolu.map((r) => r.name)).size,
    surfacesLines: lines(log, 'message="toolu: surfaces ('),
    duplicateLines: lines(log, 'message="toolu: duplicate load skipped'),
  };
  const expected = { skills: 6, distinct: 6, surfacesLines: 1, duplicateLines: 1 };
  return { pass: passes(observed, expected), observed };
}

async function selectionSources(ctx: EntryContext): Promise<EntryResult> {
  const ids = catalogIds();
  const scripts: Scripts = {
    "surfaces.selection": [{ tool: "bash", args: { command: "touch ran.txt", description: "x" } }],
  };
  using s = install(ctx, {}, scripts, () => ({ permission: { bash: "allow" } }));
  const globalFile = join(s.sb.home, ".config/opencode/toolu/plugins.json");
  mkdirSync(dirname(globalFile), { recursive: true });
  writeFileSync(globalFile, selection(["jev"]));
  const global = await skills(ctx, s);
  s.sb.write(SELECTION, selection(["toolu-review"]));
  const project = await skills(ctx, s);
  s.sb.write(SELECTION, "{ not json");
  const invalid = await skills(ctx, s);
  const refused = (await runHost(ctx.bin, s, ["PROBE:surfaces.selection"])).events;
  const bash = toolResults(refused).find((r) => r.tool === "bash");
  rmSync(globalFile);
  const observed = {
    global: named(global.rows, ids.skills).join(","),
    globalLogged: lines(global.log, "global selection") > 0,
    project: named(project.rows, ids.skills).join(","),
    invalidSkills: named(invalid.rows, ids.skills).length,
    refused: bash?.text.includes("toolu: not ready: plugin selection: invalid") === true,
    notRun: !s.exists("ran.txt"),
  };
  const expected = {
    global: "jev-jev",
    globalLogged: true,
    project: "toolu-review-review",
    invalidSkills: 0,
    refused: true,
    notRun: true,
  };
  return { pass: passes(observed, expected), observed };
}

export const INSTALL_SCENARIOS: EntryScenario[] = [
  {
    id: "surfaces.npm-clean",
    claim:
      "A clean npm install exposes exactly the selection's skills (native skill tool), agents and commands, and a linked shared procedure is readable",
    run: npmClean,
  },
  {
    id: "surfaces.lifecycle",
    claim: "Reselect, update and remove change only toolu's surfaces and write no files",
    run: lifecycle,
  },
  {
    id: "surfaces.precedence",
    claim: "User skills, agents, commands and permission rules win over toolu's",
    run: precedence,
  },
  {
    id: "surfaces.skill-roots",
    claim:
      "A user skill in any host skill root leaves exactly one copy; disabled and dot roots do not count",
    run: skillRoots,
  },
  {
    id: "surfaces.both-routes",
    claim: "The npm route and a local shim together contribute each surface once",
    run: bothRoutes,
  },
  {
    id: "surfaces.selection",
    claim: "Global, then project selection decide; an invalid project selection fails closed",
    run: selectionSources,
  },
];
