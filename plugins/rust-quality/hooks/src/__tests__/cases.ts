/** Every rust-quality golden case (#267), loaded from shared JSON. */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { readQualityCases } from "@toolu/conformance/harness/quality-cases";
import { z } from "zod";
import type { RsCase } from "./cases-types.ts";

const ROOT = resolve(import.meta.dir, "../../../../../fixtures/quality");
export const RS_CASES: readonly RsCase[] = readQualityCases(resolve(ROOT, "rust.json"));

const ExcludedSchema = z.strictObject({ version: z.literal(1), names: z.array(z.string()) });
export const NOT_GOLDEN: readonly string[] = ExcludedSchema.parse(
  JSON.parse(readFileSync(resolve(ROOT, "rust-excluded.json"), "utf8")),
).names;
