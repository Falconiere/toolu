/** Normalized event schema placeholder (#210). */
import { z } from "zod";

export const NormalizedEventSchema = z.object({
  type: z.string().min(1),
  payload: z.unknown(),
});

export type NormalizedEvent = z.infer<typeof NormalizedEventSchema>;

/** Parse a normalized event. */
export function parseNormalizedEvent(input: unknown): NormalizedEvent {
  return NormalizedEventSchema.parse(input);
}
