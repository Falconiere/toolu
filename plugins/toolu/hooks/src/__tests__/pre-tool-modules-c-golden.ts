/**
 * The bash captures of #262 (`fixtures/pre-tool-modules-c-golden.json`): what
 * the base PreToolUse bundle (push-review, plan-ledger and docs-sync still on
 * bash) and `agent-tier.sh` printed and wrote for each case at 2912cd9d,
 * before the bash was deleted.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { AGENT_TIER_CASES } from "./pre-tool-modules-c-agent-tier.ts";
import type { Captured, GateCase } from "./pre-tool-modules-c-cases.ts";
import { DOCS_SYNC_CASES } from "./pre-tool-modules-c-docs-sync.ts";
import { PLAN_LEDGER_CASES } from "./pre-tool-modules-c-plan-ledger.ts";
import { PUSH_REVIEW_CASES } from "./pre-tool-modules-c-push-review.ts";
import { CASES_283 } from "./pre-tool-modules-c-283.ts";

export const BASE_COMMIT = "2912cd9d";

export const GOLDEN_PATH = join(import.meta.dir, "fixtures", "pre-tool-modules-c-golden.json");

export const MODULE_CASES: readonly GateCase[] = [
  ...PUSH_REVIEW_CASES,
  ...PLAN_LEDGER_CASES,
  ...DOCS_SYNC_CASES,
  ...AGENT_TIER_CASES,
  ...CASES_283,
];

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
  return GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
}
