/** Discriminated decision union (portable-core #210). */
import { z } from "zod";

export const DecisionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("allow") }),
  z.object({ kind: z.literal("ask"), reason: z.string().min(1) }),
  z.object({ kind: z.literal("deny"), reason: z.string().min(1) }),
  z.object({ kind: z.literal("advisory"), message: z.string().min(1) }),
  z.object({ kind: z.literal("post_block"), reason: z.string().min(1) }),
  z.object({
    kind: z.literal("runtime_failure"),
    reason: z.string().min(1),
    code: z.enum(["timeout", "spawn", "parse", "truncated", "cancelled", "nonzero"]),
  }),
]);

export type Decision = z.infer<typeof DecisionSchema>;

/** Parse an unknown decision envelope. */
export function parseDecision(input: unknown): Decision {
  return DecisionSchema.parse(input);
}
