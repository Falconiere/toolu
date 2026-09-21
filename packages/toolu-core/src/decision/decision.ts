/** Discriminated decision union placeholder (#210 fills real variants). */
import { z } from "zod";

export const DecisionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("allow") }),
  z.object({ kind: z.literal("deny"), reason: z.string().min(1) }),
]);

export type Decision = z.infer<typeof DecisionSchema>;

/** Parse an unknown decision envelope. */
export function parseDecision(input: unknown): Decision {
  return DecisionSchema.parse(input);
}
