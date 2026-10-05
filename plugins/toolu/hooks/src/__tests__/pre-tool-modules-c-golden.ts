/**
 * The bash captures of #262 (`fixtures/pre-tool-modules-c-golden.json`): what
 * the base PreToolUse bundle (push-review, plan-ledger and docs-sync still on
 * bash) and `agent-tier.sh` printed and wrote for each case at 2912cd9d,
 * before the bash was deleted.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { assertCaptureNames, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";
import { MODULE_CASES, type Captured } from "./pre-tool-modules-c-cases.ts";

export const BASE_COMMIT = "2912cd9d";

export const GOLDEN_PATH = resolve(
  import.meta.dir,
  "../../../../../fixtures/gates/pre-tool-modules-c-golden.json",
);

export { MODULE_CASES };

const CapturedSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number(),
  files: z.record(z.string(), z.string().nullable()),
});

const GoldenSchema = z.strictObject({
  base: z.string(),
  /** Keyed by `GateCase.name`. */
  cases: z.record(z.string(), CapturedSchema),
});

export type Golden = { base: string; cases: Record<string, Captured> };

export function readGolden(): Golden {
  const golden = GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
  assertCaptureNames(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-c.json")),
    golden.cases,
  );
  return golden;
}
