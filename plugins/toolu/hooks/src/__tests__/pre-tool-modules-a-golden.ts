/** Reader for `fixtures/pre-tool-modules-a-golden.json`, the bash captures of #260. */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { assertCaptureNames, readCaseFile } from "@toolu/conformance/harness/json-cases";
import { z } from "zod";

const CapturedSchema = z.strictObject({
  stdout: z.string(),
  stderr: z.string(),
  exitCode: z.number(),
});

const GoldenSchema = z.strictObject({
  base: z.string(),
  /** Keyed by `ModuleCase.name`. */
  cases: z.record(z.string(), CapturedSchema),
  /** #258 corpus cases these modules decide, keyed `<fixture name> [<host>]`. */
  corpus: z.record(z.string(), CapturedSchema),
});

export type Golden = z.infer<typeof GoldenSchema>;

export function readGolden(): Golden {
  const path = resolve(
    import.meta.dir,
    "../../../../../fixtures/gates/pre-tool-modules-a-golden.json",
  );
  const golden = GoldenSchema.parse(JSON.parse(readFileSync(path, "utf8")));
  assertCaptureNames(
    readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/gates/pre-tool-modules-a.json")),
    golden.cases,
  );
  return golden;
}

export function corpusKey(name: string, host: string): string {
  return `${name} [${host}]`;
}
