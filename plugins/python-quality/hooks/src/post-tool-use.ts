/**
 * python-quality's PostToolUse registry module (#266): the post-edit Python
 * quality rules, run in process by toolu's dispatcher for every edited `.py`
 * file of a Python project with `python3` on PATH. Violations fail the file's
 * quality-gate entry; the docs advisory only informs. Replaces the 8 bash
 * concern fragments `register.sh` assembled.
 */
import { readFileSync } from "node:fs";
import { loadConfig, qualityFlag, qualityThreshold } from "@toolu/core/config";
import { detectPython, toolAvailable } from "@toolu/core/detect";
import { fileQuality, type EditedFile } from "@toolu/core/quality";
import { defineRegistryModule, type RegistryContext } from "@toolu/core/registry";
import { checkPyFile } from "./rules/check.ts";
import { splitLines, type PyLimits } from "./rules/py-file.ts";

const ALLOW = { kind: "allow" } as const;

function limitsFor(ctx: RegistryContext): PyLimits {
  const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => undefined });
  return {
    fileLines: qualityThreshold(config, "python", "maxFileLines", where),
    fnLines: qualityThreshold(config, "python", "maxFnLines", where),
    noMocks: qualityFlag(config, "python", "noMocks", true),
  };
}

function read(file: EditedFile): { text: string; error?: never } | { text?: never; error: string } {
  try {
    return { text: readFileSync(file.absolute, "utf8") };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { error: `Cannot read ${file.path} for Python quality checks: ${detail}` };
  }
}

export default defineRegistryModule({
  spec: "python-quality@toolu",
  name: "python-quality",
  event: "tool/post",
  run(event, ctx) {
    const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
    if (!detectPython(where) || !toolAvailable("python3", ctx.env)) return Promise.resolve(ALLOW);
    let limits: PyLimits | undefined;
    const decision = fileQuality(event, ctx, {
      source: "python-quality-hook",
      reason: "Post-edit Python quality violation(s) detected",
      matches: /\.py$/,
      skipLinkedWorktrees: false,
      check: (file) => {
        const source = read(file);
        if (source.error !== undefined) return { errors: [source.error], advisories: [] };
        limits ??= limitsFor(ctx);
        return checkPyFile({ file, lines: splitLines(source.text), ctx, limits });
      },
    });
    return Promise.resolve(decision);
  },
});
