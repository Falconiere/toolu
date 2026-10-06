/** Reader for `fixtures/lifecycle-golden.json`, the bash captures of #263. */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { assertCaptureNames, readCaseFile } from "@toolu/conformance/harness/json-cases";
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
  const path = resolve(import.meta.dir, "../../../../../fixtures/gates/lifecycle-golden.json");
  const golden = GoldenSchema.parse(JSON.parse(readFileSync(path, "utf8")));
  assertCaptureNames(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/lifecycle.json")),
    golden.cases,
  );
  return golden;
}
