/**
 * ts-quality's PostToolUse registry module (#265): the post-edit TypeScript
 * quality rules, run in process by toolu's dispatcher for every edited `.ts`
 * or `.tsx` file of a TypeScript project with a package manager. Violations
 * fail the file's quality-gate entry; advisories only inform. Replaces the 24
 * bash concern fragments `register.sh` assembled.
 */
import { readFileSync } from "node:fs";
import {
  loadConfig,
  qualityFlag,
  qualityThreshold,
  tsMaxFileLinesResolved,
} from "@toolu/core/config";
import { detectTs, nodePackageManager, toolAvailable } from "@toolu/core/detect";
import { fileQuality, type EditedFile } from "@toolu/core/quality";
import { defineRegistryModule, type RegistryContext } from "@toolu/core/registry";
import { checkTsFile } from "./rules/check.ts";
import { splitLines, type TsLimits } from "./rules/ts-file.ts";

const ALLOW = { kind: "allow" } as const;

function limitsFor(ctx: RegistryContext): TsLimits {
  const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => undefined });
  const file = tsMaxFileLinesResolved(config, where);
  return {
    fileLines: file.value,
    fileSource: file.source,
    fnLines: qualityThreshold(config, "ts", "maxFnLines", where),
    noMocks: qualityFlag(config, "ts", "noMocks", true),
  };
}

function read(file: EditedFile): string {
  try {
    return readFileSync(file.absolute, "utf8");
  } catch {
    return "";
  }
}

export default defineRegistryModule({
  spec: "ts-quality@toolu",
  name: "ts-quality",
  event: "tool/post",
  run(event, ctx) {
    const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
    if (!detectTs(where)) return Promise.resolve(ALLOW);
    const pm = nodePackageManager(where);
    if (pm === undefined || !toolAvailable(pm, ctx.env)) return Promise.resolve(ALLOW);
    const limits = limitsFor(ctx);
    const decision = fileQuality(event, ctx, {
      source: "ts-quality-hook",
      reason: "Post-edit quality violation(s) detected",
      matches: /\.(ts|tsx)$/s,
      skipLinkedWorktrees: true,
      check: (file) => checkTsFile({ file, lines: splitLines(read(file)), ctx, limits, pm }),
    });
    return Promise.resolve(decision);
  },
});
