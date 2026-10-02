/**
 * The complete live scenario list (#335), one per `PROBE_IDS` entry and in
 * that order, so the committed results file has a stable shape.
 */
import type { Scenario } from "./scenario.ts";
import { CONTEXT_SCENARIOS } from "./scenarios-context.ts";
import { LOAD_SCENARIOS } from "./scenarios-load.ts";
import { PERMISSION_SCENARIOS } from "./scenarios-permission.ts";
import { POST_SCENARIOS } from "./scenarios-post.ts";
import { SURFACE_SCENARIOS } from "./scenarios-surface.ts";
import { TOOL_SCENARIOS } from "./scenarios-tools.ts";
import { PROBE_IDS } from "./schema.ts";

const BY_ID = new Map(
  [
    ...LOAD_SCENARIOS,
    ...TOOL_SCENARIOS,
    ...PERMISSION_SCENARIOS,
    ...POST_SCENARIOS,
    ...CONTEXT_SCENARIOS,
    ...SURFACE_SCENARIOS,
  ].map((s) => [s.id, s]),
);

export const SCENARIOS: Scenario[] = PROBE_IDS.map((id) => {
  const scenario = BY_ID.get(id);
  if (scenario === undefined) throw new Error(`no live scenario for probe ${id}`);
  return scenario;
});
