/**
 * The bash captures of #261 (`fixtures/pre-tool-modules-b-golden.json`): what
 * `bash pre-tools/mod.sh` printed for each module case and for each #258 corpus
 * fixture these modules decide, at the base commit, before the bash modules
 * were deleted.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { assertCaptureNames, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import { MODULE_CASES, type Captured } from "./pre-tool-modules-b-cases.ts";

export const GOLDEN_PATH = resolve(
  import.meta.dir,
  "../../../../../fixtures/gates/pre-tool-modules-b-golden.json",
);

export { MODULE_CASES };

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
  const golden = GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
  assertCaptureNames(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-b.json")),
    golden.cases,
  );
  return golden;
}
