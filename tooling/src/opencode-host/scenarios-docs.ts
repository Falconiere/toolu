/**
 * Live documentation scenarios (#363): the OpenCode install guide and the
 * migration guide, run word for word on the pinned host.
 *
 * `docs.quickstart` starts from a clean profile, runs docs/opencode.md's quick
 * start (install toolu and a leaf plugin, discover its skill, watch a `.env`
 * write be refused), then its update-and-remove block. `docs.migration` seeds
 * the state a V2-era install left behind, runs docs/opencode-migration.md's
 * migrate block, checks toolu loads once and enforces, then runs its rollback
 * block and checks every seeded file is restored byte for byte.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { z } from "zod";
import { docBlock, runDocBlock } from "./doc-commands.ts";
import { mergedPlugins, tooluSkills } from "./scenarios-cli.ts";
import {
  PROJECT_FILES,
  ROOT,
  enforced,
  entrySession,
  npmSpec,
  protectedWrite,
  writeEnvScript,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { PROBE_PLUGIN, type ProbeSession } from "./session.ts";

const INSTALL_DOC = join(ROOT, "docs/opencode.md");
const MIGRATION_DOC = join(ROOT, "docs/opencode-migration.md");
const PACKAGE = "@toolu/opencode";
const ORIGINAL_ENV = "SECRET=1\n";
/** Text of the protected-files refusal the model must receive as the tool error. */
const REFUSAL = ["a protected path", "Blocked by gates.protectedFiles.mode='block'"];
const Selection = z.object({ version: z.literal(1), enabled: z.array(z.string()) });

function block(doc: string, name: string): string {
  return docBlock(readFileSync(doc, "utf8"), name);
}

function globalDir(s: ProbeSession): string {
  return join(s.sb.home, ".config/opencode");
}

function enabled(path: string): string {
  if (!existsSync(path)) return "absent";
  return Selection.parse(JSON.parse(readFileSync(path, "utf8")))
    .enabled.toSorted()
    .join(",");
}

/** Whether the scripted model's requests carry toolu's refusal back as a tool result. */
function refusalReachedModel(s: ProbeSession): boolean {
  return s
    .requests()
    .some((request) => REFUSAL.every((text) => JSON.stringify(request.body).includes(text)));
}

const tooluEntries = (merged: readonly string[]): string[] =>
  merged.filter((entry) => entry.includes(PACKAGE));

async function quickstart(ctx: EntryContext): Promise<EntryResult> {
  // A clean project: no selection, no gate config, no .env; only the session's provider config.
  using s = entrySession(ctx, {
    files: {},
    scripts: (project) => ({
      "*": [{ tool: "write", args: { filePath: join(project, ".env"), content: "PWNED\n" } }],
    }),
  });
  const start = await runDocBlock(ctx, s, block(INSTALL_DOC, "quickstart"));
  const merged = tooluEntries(await mergedPlugins(ctx, s));
  const skills = await tooluSkills(ctx, s);
  const afterStart = {
    startExit: start.exitCode,
    refusedLine: start.stdout.includes("toolu refused the write"),
    entries: merged.length,
    spec: merged[0] === JSON.stringify(npmSpec(ctx.tarball)),
    selection: enabled(join(globalDir(s), "toolu/plugins.json")),
    leafSkill: skills.includes("context7-context7"),
    envUnchanged: s.exists(".env") && s.sb.read(".env") === ORIGINAL_ENV,
    refusalReachedModel: refusalReachedModel(s),
  };
  const manage = await runDocBlock(ctx, s, block(INSTALL_DOC, "manage"));
  const observed = {
    ...afterStart,
    manageExit: manage.exitCode,
    updateCurrent: /current/.test(manage.stdout + manage.stderr),
    entriesAfterRemove: tooluEntries(await mergedPlugins(ctx, s)).length,
    skillsAfterRemove: (await tooluSkills(ctx, s)).length,
    selectionAfterRemove: enabled(join(globalDir(s), "toolu/plugins.json")),
    tail: `${start.stderr.slice(-300)}${manage.stderr.slice(-300)}`,
  };
  const pass =
    observed.startExit === 0 &&
    observed.refusedLine &&
    observed.entries === 1 &&
    observed.spec &&
    observed.selection === "context7,toolu" &&
    observed.leafSkill &&
    observed.envUnchanged &&
    observed.refusalReachedModel &&
    observed.manageExit === 0 &&
    observed.updateCurrent &&
    observed.entriesAfterRemove === 0 &&
    observed.skillsAfterRemove === 0 &&
    observed.selectionAfterRemove === "toolu";
  return { pass, observed };
}

/** The global config a V2-era `opencode plugin add @toolu/opencode` left, with the user's own lines. */
function v2GlobalConfig(): string {
  return [
    "// my OpenCode settings",
    "{",
    "  // added by the V2 plugin command, next to a plugin of my own",
    `  "plugin": ["${PACKAGE}", ${JSON.stringify(pathToFileURL(PROBE_PLUGIN).href)}],`,
    '  "share": "disabled",',
    "}",
    "",
  ].join("\n");
}

/** The V2-era project files: the clone shim and its dependencies, plus the user's selection and gate config. */
const V2_PROJECT = {
  ".opencode/plugins/toolu.ts": 'export { default } from "@toolu/opencode";\n',
  ".opencode/package.json": `${JSON.stringify(
    {
      dependencies: {
        "@opencode/plugin": "2.0.12",
        "@toolu/opencode": "file:../toolu/tools/toolu-opencode",
        "@toolu/core": "file:../toolu/packages/toolu-core",
      },
    },
    null,
    2,
  )}\n`,
};
const KEPT = [".opencode/toolu/plugins.json", ".opencode/toolu.config.json"];
const GLOBAL_FILE = "opencode.jsonc";

function snapshot(s: ProbeSession): Record<string, string> {
  const project = [...KEPT, ...Object.keys(V2_PROJECT)];
  return {
    ...Object.fromEntries(project.map((rel) => [rel, s.exists(rel) ? s.sb.read(rel) : "absent"])),
    [GLOBAL_FILE]: readFileSync(join(globalDir(s), GLOBAL_FILE), "utf8"),
  };
}

const V2_DEPENDENCIES = new Set(["@opencode/plugin", "@toolu/opencode", "@toolu/core"]);
const Dependencies = z.looseObject({ dependencies: z.record(z.string(), z.string()).optional() });

/** The V2 dependencies still in `.opencode/package.json`; the host may have added its own. */
function v2DependenciesLeft(s: ProbeSession): number {
  const deps = Dependencies.parse(JSON.parse(s.sb.read(".opencode/package.json"))).dependencies;
  return Object.keys(deps ?? {}).filter((name) => V2_DEPENDENCIES.has(name)).length;
}

async function migration(ctx: EntryContext): Promise<EntryResult> {
  using s = entrySession(ctx, {
    files: { ...PROJECT_FILES, ...V2_PROJECT },
    config: () => ({ permission: { bash: "allow" } }),
    scripts: writeEnvScript,
  });
  mkdirSync(globalDir(s), { recursive: true });
  writeFileSync(join(globalDir(s), GLOBAL_FILE), v2GlobalConfig());
  const seeded = snapshot(s);
  const migrate = await runDocBlock(ctx, s, block(MIGRATION_DOC, "migrate"));
  const globalText = readFileSync(join(globalDir(s), GLOBAL_FILE), "utf8");
  const merged = tooluEntries(await mergedPlugins(ctx, s));
  const gate = await protectedWrite(ctx, s);
  const afterMigrate = {
    migrateExit: migrate.exitCode,
    entries: merged.length,
    spec: merged[0] === JSON.stringify(npmSpec(ctx.tarball)),
    commentsKept:
      globalText.includes("// my OpenCode settings") && globalText.includes("next to a plugin"),
    otherPluginKept: globalText.includes(pathToFileURL(PROBE_PLUGIN).href),
    shimRemoved: !s.exists(".opencode/plugins/toolu.ts"),
    v2DependenciesLeft: v2DependenciesLeft(s),
    keptChanged: KEPT.filter((rel) => s.sb.read(rel) !== seeded[rel]).join(","),
    enforced: enforced(gate),
    ...gate,
  };
  const rollback = await runDocBlock(ctx, s, block(MIGRATION_DOC, "rollback"));
  const restored = snapshot(s);
  const observed = {
    ...afterMigrate,
    rollbackExit: rollback.exitCode,
    restored: Object.keys(seeded).every((key) => restored[key] === seeded[key]),
    tail: `${migrate.stderr.slice(-300)}${rollback.stderr.slice(-300)}`,
  };
  const pass =
    observed.migrateExit === 0 &&
    observed.entries === 1 &&
    observed.spec &&
    observed.commentsKept &&
    observed.otherPluginKept &&
    observed.shimRemoved &&
    observed.v2DependenciesLeft === 0 &&
    observed.keptChanged === "" &&
    gate.ready === 1 &&
    gate.duplicate === 0 &&
    observed.enforced &&
    observed.rollbackExit === 0 &&
    observed.restored;
  return { pass, observed };
}

export const QUICKSTART_SCENARIOS: EntryScenario[] = [
  {
    id: "docs.quickstart",
    claim:
      "docs/opencode.md's quick start, run verbatim in a clean profile, installs toolu and context7, discovers context7's skill and refuses a .env write; its update/remove block then leaves no toolu entry or skill",
    run: quickstart,
  },
];

export const MIGRATION_SCENARIOS: EntryScenario[] = [
  {
    id: "docs.migration",
    claim:
      "docs/opencode-migration.md's migrate block, run verbatim over V2-era state, loads toolu once and enforces while keeping user files; its rollback block restores every seeded file",
    run: migration,
  },
];

export const DOCS_SCENARIOS: EntryScenario[] = [...QUICKSTART_SCENARIOS, ...MIGRATION_SCENARIOS];
