/** Reader for `fixtures/golden.json`, the bash captures of #268. */
import { readFileSync } from "node:fs";
import { z } from "zod";
import { GOLDEN_PATH } from "./golden-sandbox.ts";

const CapturedSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number(),
});

const GoldenSchema = z.strictObject({
  base: z.string().regex(/^[0-9a-f]{40}$/),
  /** Keyed `<case name> [<host>]`. */
  nudge: z.record(z.string(), CapturedSchema),
  savings: z.record(
    z.string(),
    CapturedSchema.extend({ ledgers: z.record(z.string(), z.string()) }),
  ),
  /** Keyed by case name. */
  report: z.record(z.string(), CapturedSchema),
  /** #258 PreToolUse corpus fixtures search-nudge decides, keyed `<fixture name> [<host>]`. */
  corpus: z.record(z.string(), CapturedSchema),
});

export type Golden = z.infer<typeof GoldenSchema>;

export function readGolden(): Golden {
  return GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
}
