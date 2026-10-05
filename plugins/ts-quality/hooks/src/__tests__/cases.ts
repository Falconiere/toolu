/** Every ts-quality golden case (#265), loaded from shared JSON. */
import { resolve } from "node:path";
import { readQualityCases } from "@toolu/conformance/harness/quality-cases";
import type { TsCase } from "./cases-types.ts";

export const TS_CASES: readonly TsCase[] = readQualityCases(
  resolve(import.meta.dir, "../../../../../fixtures/quality/ts.json"),
);
