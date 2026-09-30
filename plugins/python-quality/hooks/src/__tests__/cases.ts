/** Every python-quality golden case (#266), in bats-suite order. */
import { EDGE_CASES } from "./cases-edges.ts";
import { FLOW_CASES } from "./cases-flow.ts";
import { MOCK_CASES } from "./cases-mocks.ts";
import { RULE_CASES } from "./cases-rules.ts";
import type { PyCase } from "./cases-types.ts";

export const PY_CASES: readonly PyCase[] = [
  ...FLOW_CASES,
  ...RULE_CASES,
  ...MOCK_CASES,
  ...EDGE_CASES,
];

/**
 * Named deviations (spec DEV-1): the golden records bash's result, the
 * TypeScript module must give this instead.
 */
export const DEVIATIONS: Readonly<Record<string, { contains: readonly string[] }>> = {
  "DEV-1: no jq on PATH": { contains: ["Forbidden suppression"] },
};
