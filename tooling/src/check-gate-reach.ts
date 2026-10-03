/**
 * Gate reach (`bun run check:gate-reach`): every tracked TypeScript file is
 * reached by typecheck, format, oxlint, jscpd and knip, or tooling/gate-reach.json
 * allows the gap, and every check a guardrails package hands to the linter is a
 * rule the linter runs. Exit 0 clean, 1 findings, 3 misconfigured.
 * `GATE_REACH_ROOT` points it elsewhere.
 */
import { gateMain } from "./gate-reach/gate-main.ts";
import { checkOwnedRules } from "./gate-reach/owned-rules.ts";
import { checkReach } from "./gate-reach/reach-run.ts";

if (import.meta.main) {
  process.exitCode = gateMain("gate-reach", "GATE_REACH_ROOT", (root) => [
    ...checkReach(root),
    ...checkOwnedRules(root),
  ]);
}
