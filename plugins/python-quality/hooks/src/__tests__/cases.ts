/** Every python-quality golden case (#266), loaded from shared JSON. */
import { resolve } from "node:path";
import { readQualityCases } from "@toolu/conformance/harness/quality-cases";
import type { PyCase } from "./cases-types.ts";

export const PY_CASES: readonly PyCase[] = readQualityCases(
  resolve(import.meta.dir, "../../../../../fixtures/quality/python.json"),
);
