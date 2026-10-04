/**
 * Live migration refusal scenario (#363): docs/opencode-migration.md's migrate
 * block where `update` must fail. A clone-only setup exits 1 and a package in
 * both scopes exits 2; each leaves every user file unchanged. The clone-only
 * setups then run the documented no-entry step, the block again and the
 * rollback.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { MIGRATION_DOC, PACKAGE, block, globalDir, runDocBlock } from "./doc-commands.ts";
import {
  GLOBAL_FILE,
  V2_PROJECT,
  changedSince,
  listed,
  snapshot,
  v2GlobalConfig,
} from "./scenarios-docs-migration.ts";
import {
  GATED_FILES,
  entrySession,
  npmSpec,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

/** A V2-era setup: a global V2 entry, a project entry, and the project selection (`null` for no file). */
type Seed = { global: boolean; projectEntry: boolean; selection: readonly string[] | null };
type Refusal = { exit: number; message: boolean; changed: string; recovered: string };

const SELECTION = ".opencode/toolu/plugins.json";
const Selection = z.object({ version: z.literal(1), enabled: z.array(z.string()) });

/** Each backup tarball's bytes, keyed by path. */
function backupBytes(s: ProbeSession): Record<string, string> {
  const root = join(s.sb.home, "toolu-opencode-v2-backup");
  const files = [join(root, "global.tgz"), ...listed(join(root, "projects"))].filter(existsSync);
  return Object.fromEntries(files.map((file) => [file, readFileSync(file).toString("base64")]));
}

/** The project selection's names, sorted and joined, or "absent". */
function projectSelection(s: ProbeSession): string {
  if (!s.exists(SELECTION)) return "absent";
  return Selection.parse(JSON.parse(s.sb.read(SELECTION)))
    .enabled.toSorted()
    .join(",");
}

/** What the no-entry step must leave: the seed's selection, with `toolu` added when one exists. */
function expectedSelection(seed: Seed): string {
  if (seed.selection === null) return "absent";
  return [...new Set([...seed.selection, "toolu"])].toSorted().join(",");
}

/** Whether any config file the host merges still names toolu's package. */
function entryLeft(s: ProbeSession): boolean {
  const files = [
    join(globalDir(s), "opencode.json"),
    join(globalDir(s), GLOBAL_FILE),
    join(s.sb.project, "opencode.json"),
  ];
  return files.some((file) => existsSync(file) && readFileSync(file, "utf8").includes(PACKAGE));
}

/**
 * After a refused migrate block, run the documented no-entry step, the block
 * again, then the rollback; the names of the expectations that failed, or "".
 */
async function recover(
  ctx: EntryContext,
  s: ProbeSession,
  seed: Seed,
  step: string,
  seeded: Record<string, string>,
): Promise<string> {
  const firstBackups = JSON.stringify(backupBytes(s));
  const added = await runDocBlock(ctx, s, block(MIGRATION_DOC, step));
  const again = await runDocBlock(ctx, s, block(MIGRATION_DOC, "migrate"));
  const globalConfig = join(globalDir(s), "opencode.json");
  const checks: Record<string, boolean> = {
    stepExit: added.exitCode === 0,
    rerunExit: again.exitCode === 0,
    entry:
      existsSync(globalConfig) &&
      readFileSync(globalConfig, "utf8").includes(JSON.stringify(npmSpec(ctx.tarball))),
    shimRemoved: !s.exists(".opencode/plugins/toolu.ts"),
    selection: projectSelection(s) === expectedSelection(seed),
    noGlobalSelection: !existsSync(join(globalDir(s), "toolu/plugins.json")),
    backupsKept: JSON.stringify(backupBytes(s)) === firstBackups,
  };
  const rollback = await runDocBlock(ctx, s, block(MIGRATION_DOC, "rollback"));
  checks.rollbackExit = rollback.exitCode === 0;
  checks.entryRemoved = !entryLeft(s);
  checks.restored = changedSince(s, seeded) === "";
  return Object.entries(checks)
    .filter(([, ok]) => !ok)
    .map(([name]) => name)
    .join(",");
}

/**
 * Run the migrate block over `seed`, which must make `update` fail with `exit`
 * and leave every user file unchanged; then, given a no-entry `step`, recover.
 */
async function refused(
  ctx: EntryContext,
  seed: Seed,
  expected: { exit: number; message: string; step?: string },
): Promise<Refusal> {
  const selection =
    seed.selection === null
      ? {}
      : { [SELECTION]: JSON.stringify({ version: 1, enabled: seed.selection }) };
  using s = entrySession(ctx, {
    files: { ...GATED_FILES, ...selection, ...V2_PROJECT },
    config: () => (seed.projectEntry ? { plugin: [PACKAGE] } : {}),
  });
  if (seed.global) {
    mkdirSync(globalDir(s), { recursive: true });
    writeFileSync(join(globalDir(s), GLOBAL_FILE), v2GlobalConfig());
  }
  const seeded = snapshot(s);
  const res = await runDocBlock(ctx, s, block(MIGRATION_DOC, "migrate"));
  const changed = changedSince(s, seeded);
  const recovered =
    expected.step === undefined ? "" : await recover(ctx, s, seed, expected.step, seeded);
  return {
    exit: res.exitCode,
    message: (res.stdout + res.stderr).includes(expected.message),
    changed,
    recovered,
  };
}

async function migrationRefusals(ctx: EntryContext): Promise<EntryResult> {
  const notConfigured = { exit: 1, message: `${PACKAGE} is not configured in OpenCode` };
  const cloneOnly = { global: false, projectEntry: false };
  const runs: Array<{ name: string; exit: number; run: Refusal }> = [
    {
      name: "cloneOnlySelection",
      exit: 1,
      run: await refused(
        ctx,
        { ...cloneOnly, selection: ["toolu"] },
        { ...notConfigured, step: "no-entry-selection" },
      ),
    },
    {
      name: "cloneOnlySelectionWithoutToolu",
      exit: 1,
      run: await refused(
        ctx,
        { ...cloneOnly, selection: ["context7"] },
        { ...notConfigured, step: "no-entry-selection" },
      ),
    },
    {
      name: "cloneOnlyAll",
      exit: 1,
      run: await refused(
        ctx,
        { ...cloneOnly, selection: null },
        { ...notConfigured, step: "no-entry-all" },
      ),
    },
    {
      name: "bothScopes",
      exit: 2,
      run: await refused(
        ctx,
        { global: true, projectEntry: true, selection: ["toolu"] },
        { exit: 2, message: "--scope" },
      ),
    },
  ];
  const observed: Record<string, string | number | boolean> = {};
  for (const { name, run } of runs) {
    observed[`${name}Exit`] = run.exit;
    observed[`${name}Message`] = run.message;
    observed[`${name}Changed`] = run.changed;
    observed[`${name}Unrecovered`] = run.recovered;
  }
  const pass = runs.every(
    ({ exit, run }) =>
      run.exit === exit && run.message && run.changed === "" && run.recovered === "",
  );
  return { pass, observed };
}

export const MIGRATION_REFUSAL_SCENARIOS: EntryScenario[] = [
  {
    id: "docs.migration-refusals",
    claim:
      "docs/opencode-migration.md's migrate block stops with every user file unchanged when update fails (exit 1 with no entry, exit 2 with the package in both scopes); the documented no-entry steps then let it finish, and its rollback removes toolu's entry and restores the seeded files",
    run: migrationRefusals,
  },
];
