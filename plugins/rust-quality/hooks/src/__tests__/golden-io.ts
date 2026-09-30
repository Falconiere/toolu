/** Reader for `fixtures/golden.json`, the bash captures of #267. */
import { readFileSync } from "node:fs";
import { z } from "zod";
import { GOLDEN_PATH } from "./golden-harness.ts";

const StepSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number(),
  state: z.record(z.string(), z.string()),
});

export const GoldenSchema = z.strictObject({
  base: z.string().regex(/^[0-9a-f]{40}$/),
  cases: z.record(z.string(), z.array(StepSchema)),
});

export type Golden = z.infer<typeof GoldenSchema>;

export function readGolden(): Golden {
  return GoldenSchema.parse(JSON.parse(readFileSync(GOLDEN_PATH, "utf8")));
}
