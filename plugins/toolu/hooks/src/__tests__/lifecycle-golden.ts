/** Reader for `fixtures/lifecycle-golden.json`, the bash captures of #263. */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import type { Golden } from "./lifecycle-cases.ts";

const GoldenSchema = z.strictObject({
  base: z.string(),
  cases: z.record(
    z.string(),
    z.strictObject({ stdout: z.string(), stderr: z.string(), exitCode: z.number() }),
  ),
});

export function readGolden(): Golden {
  const path = join(import.meta.dir, "fixtures", "lifecycle-golden.json");
  return GoldenSchema.parse(JSON.parse(readFileSync(path, "utf8")));
}
