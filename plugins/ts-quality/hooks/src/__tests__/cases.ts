/** Every ts-quality golden case (#265), in bats-suite order. */
import { ERROR_CASES } from "./cases-errors.ts";
import { FLOW_CASES } from "./cases-flow.ts";
import { RULE_CASES } from "./cases-rules.ts";
import { SIZE_CASES } from "./cases-size.ts";
import type { TsCase } from "./cases-types.ts";

export const TS_CASES: readonly TsCase[] = [
  ...FLOW_CASES,
  ...SIZE_CASES,
  ...RULE_CASES,
  ...ERROR_CASES,
];

/**
 * Named deviations (spec DEV-1): the golden records bash's result, the
 * TypeScript module must give this instead.
 */
export const DEVIATIONS: Readonly<Record<string, { contains: readonly string[] }>> = {
  "DEV-1: no jq on PATH": { contains: ["Forbidden console.log"] },
};
