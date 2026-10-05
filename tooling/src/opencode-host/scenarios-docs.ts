/**
 * Live quick-start scenario (#363): docs/opencode.md's quick start and its
 * update-and-remove block, run word for word on the pinned host from a clean
 * profile. It installs toolu and a leaf plugin, discovers that plugin's skill,
 * and watches a write to a scratch secrets file be refused.
 */
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import {
  INSTALL_DOC,
  PACKAGE,
  block,
  globalDir,
  runDocBlock,
  tooluEntries,
} from "./doc-commands.ts";
import { mergedPlugins, tooluSkills } from "./scenarios-cli.ts";
import {
  entrySession,
  npmSpec,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

/** The quick start's scratch secrets file, protected by the shipped `.env.*` pattern. */
const SCRATCH = ".env.toolu-check";
/** Text of the protected-files refusal that must reach the model in the tool error. */
const REFUSAL = [`${SCRATCH}, a protected path`];
/** The `update` step's report when the entry already names the release under test. */
const UPDATE_CURRENT = `current at ${PACKAGE}@`;
const Selection = z.object({ version: z.literal(1), enabled: z.array(z.string()) });

function enabled(path: string): string {
  if (!existsSync(path)) return "absent";
  return Selection.parse(JSON.parse(readFileSync(path, "utf8")))
    .enabled.toSorted()
    .join(",");
}

/** Whether a tool result in the scripted model's requests carries toolu's refusal. */
function refusalReachedModel(s: ProbeSession): boolean {
  return s
    .requests()
    .some((request) => REFUSAL.every((text) => JSON.stringify(request.body).includes(text)));
}

async function quickstart(ctx: EntryContext): Promise<EntryResult> {
  // A clean project: no selection, no gate config, no .env; only the session's provider config.
  using s = entrySession(ctx, {
    files: {},
    scripts: (project) => ({
      "*": [{ tool: "write", args: { filePath: join(project, SCRATCH), content: "PWNED\n" } }],
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
    scratchRemoved: !s.exists(SCRATCH),
    userFilesUntouched: !s.exists(".env") && !s.exists(".opencode/toolu.config.json"),
    refusalReachedModel: refusalReachedModel(s),
  };
  const manage = await runDocBlock(ctx, s, block(INSTALL_DOC, "manage"));
  const observed = {
    ...afterStart,
    manageExit: manage.exitCode,
    updateCurrent: (manage.stdout + manage.stderr).includes(UPDATE_CURRENT),
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
    observed.scratchRemoved &&
    observed.userFilesUntouched &&
    observed.refusalReachedModel &&
    observed.manageExit === 0 &&
    observed.updateCurrent &&
    observed.entriesAfterRemove === 0 &&
    observed.skillsAfterRemove === 0 &&
    observed.selectionAfterRemove === "toolu";
  return { pass, observed };
}

export const QUICKSTART_SCENARIOS: EntryScenario[] = [
  {
    id: "docs.quickstart",
    claim:
      "docs/opencode.md's quick start, run verbatim in a clean profile, installs toolu and context7, discovers context7's skill and refuses a write to the scratch .env.toolu-check; its update/remove block then leaves no toolu entry or skill",
    run: quickstart,
  },
];
