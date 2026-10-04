/**
 * Live migration scenario (#363): docs/opencode-migration.md's migrate block,
 * run word for word over the state a V2-era install left behind, then its
 * rollback block. toolu must load once and enforce while the user's files are
 * kept, and the rollback must restore every seeded file byte for byte.
 */
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { z } from "zod";
import {
  MIGRATION_DOC,
  PACKAGE,
  block,
  globalDir,
  runDocBlock,
  tooluEntries,
} from "./doc-commands.ts";
import { mergedPlugins } from "./scenarios-cli.ts";
import {
  PROJECT_FILES,
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

/** The global config a V2-era `opencode plugin add @toolu/opencode` left, with the user's own lines. */
export function v2GlobalConfig(): string {
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
export const V2_PROJECT = {
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
export const GLOBAL_FILE = "opencode.jsonc";

/** Every user file the migration may touch: project files, the project config and the global config. */
export function snapshot(s: ProbeSession): Record<string, string> {
  const project = [...KEPT, ...Object.keys(V2_PROJECT), "opencode.json"];
  const global = join(globalDir(s), GLOBAL_FILE);
  return {
    ...Object.fromEntries(project.map((rel) => [rel, s.exists(rel) ? s.sb.read(rel) : "absent"])),
    [GLOBAL_FILE]: existsSync(global) ? readFileSync(global, "utf8") : "absent",
  };
}

export function changedSince(s: ProbeSession, seeded: Record<string, string>): string {
  const now = snapshot(s);
  return Object.keys(seeded)
    .filter((key) => now[key] !== seeded[key])
    .join(",");
}

/** The backup tarballs the migrate block wrote, and whether any is readable by group or others. */
function backups(s: ProbeSession): { count: number; private: boolean } {
  const root = join(s.sb.home, "toolu-opencode-v2-backup");
  const files = [join(root, "global.tgz"), ...listed(join(root, "projects"))].filter(existsSync);
  return {
    count: files.length,
    private: files.every((file) => (statSync(file).mode & 0o077) === 0),
  };
}

export function listed(dir: string): string[] {
  return existsSync(dir) ? readdirSync(dir).map((name) => join(dir, name)) : [];
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
  const saved = backups(s);
  const afterMigrate = {
    migrateExit: migrate.exitCode,
    backups: saved.count,
    backupsPrivate: saved.private,
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
  const observed = {
    ...afterMigrate,
    rollbackExit: rollback.exitCode,
    notRestored: changedSince(s, seeded),
    dataRootRemoved: !s.exists(".opencode/toolu/state"),
    tail: `${migrate.stderr.slice(-300)}${rollback.stderr.slice(-300)}`,
  };
  const pass =
    observed.migrateExit === 0 &&
    observed.backups === 2 &&
    observed.backupsPrivate &&
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
    observed.notRestored === "" &&
    observed.dataRootRemoved;
  return { pass, observed };
}

export const MIGRATION_SCENARIOS: EntryScenario[] = [
  {
    id: "docs.migration",
    claim:
      "docs/opencode-migration.md's migrate block, run verbatim over V2-era state, loads toolu once and enforces while keeping user files; its rollback block restores every seeded file",
    run: migration,
  },
];
