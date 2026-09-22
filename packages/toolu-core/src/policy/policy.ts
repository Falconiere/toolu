/** Classification enum + decision precedence (#210). */
import { z } from "zod";
import { type Decision, DecisionSchema } from "../decision/decision.ts";

export const ClassificationSchema = z.enum(["shell-out", "port-native", "port-new", "no-map"]);

export type Classification = z.infer<typeof ClassificationSchema>;

/** Parse a portable-core classification token. */
export function parseClassification(input: unknown): Classification {
  return ClassificationSchema.parse(input);
}

const MERGE_RANK: Record<Decision["kind"], number> = {
  runtime_failure: 6,
  deny: 5,
  post_block: 5,
  ask: 4,
  advisory: 3,
  allow: 1,
};

/** deny > ask > advisory; allow loses to any of those. */
export function mergeDecisions(decisions: Decision[]): Decision {
  if (decisions.length === 0) {
    return { kind: "allow" };
  }
  let best = decisions[0];
  if (best === undefined) {
    return { kind: "allow" };
  }
  for (let i = 1; i < decisions.length; i++) {
    const next = decisions[i];
    if (next === undefined) {
      continue;
    }
    if (MERGE_RANK[next.kind] > MERGE_RANK[best.kind]) {
      best = next;
    }
  }
  return DecisionSchema.parse(best);
}
