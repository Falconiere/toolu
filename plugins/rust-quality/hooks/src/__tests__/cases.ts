/** Every rust-quality golden case (#267), in bats-suite order. */
import { ERROR_CASES } from "./cases-errors.ts";
import { FLOW_CASES } from "./cases-flow.ts";
import { LAYOUT_CASES } from "./cases-layout.ts";
import { RULE_CASES } from "./cases-rules.ts";
import { SIZE_CASES } from "./cases-size.ts";
import type { RsCase } from "./cases-types.ts";

export const RS_CASES: readonly RsCase[] = [
  ...FLOW_CASES,
  ...LAYOUT_CASES,
  ...SIZE_CASES,
  ...RULE_CASES,
  ...ERROR_CASES,
];

/**
 * Named deviations (spec DEV-1): the golden records bash's result, the
 * TypeScript module must give this instead.
 */
export const DEVIATIONS: Readonly<Record<string, { contains: readonly string[] }>> = {
  "DEV-1: no jq on PATH": { contains: ["Forbidden lint suppression"] },
};

/**
 * Bats tests with no golden case: they test the bash assembly itself (the
 * dispatcher's `TOOLU_LIB_DIR` hand-off, the assembled `.sh` file), which the
 * TypeScript module does not have. register.bats is ported by register.test.ts.
 */
export const NOT_GOLDEN: readonly string[] = [
  "dispatch.bats: rust-quality: exits 0 silently when TOOLU_LIB_DIR is unset (fail soft)",
  "assembled.bats: assembled module exists after register.sh",
];
