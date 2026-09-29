/**
 * The bash captures of #261 (`fixtures/pre-tool-modules-b-golden.json`): what
 * `bash pre-tools/mod.sh` printed for each module case and for each #258 corpus
 * fixture these modules decide, at the base commit, before the bash modules
 * were deleted.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { BASH_COMMANDS_CASES } from "./pre-tool-modules-b-bash-commands.ts";
import type { Captured, ModuleCase } from "./pre-tool-modules-b-cases.ts";
import { COMMIT_GATE_CASES } from "./pre-tool-modules-b-commit-gate.ts";
import { QUALITY_GATE_CASES } from "./pre-tool-modules-b-quality-gate.ts";

export const GOLDEN_PATH = join(import.meta.dir, "fixtures", "pre-tool-modules-b-golden.json");

export const MODULE_CASES: readonly ModuleCase[] = [
  ...BASH_COMMANDS_CASES,
  ...COMMIT_GATE_CASES,
  ...QUALITY_GATE_CASES,
];

/** #258 corpus fixtures these modules decide: their names start with the module's. */
export function decidedByModulesB(fixtureName: string): boolean {
  return ["bash-commands:", "commit-gate:", "quality-gate:"].some((p) => fixtureName.startsWith(p));
}

export function corpusKey(name: string, host: string): string {
  return `${name} [${host}]`;
}

const CapturedSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number(),
});

const GoldenSchema = z.strictObject({
  base: z.string(),
  /** Keyed by `ModuleCase.name`. */
  cases: z.record(z.string(), CapturedSchema),
  /** Keyed by `corpusKey(<fixture name>, <host>)`. */
  corpus: z.record(z.string(), CapturedSchema),
});

export type Golden = {
  base: string;
  cases: Record<string, Captured>;
  corpus: Record<string, Captured>;
};

export function readGolden(): Golden {
  return GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
}
