/** Bash bridge request/response placeholder (#210). */
import { z } from "zod";

export const BridgeRequestSchema = z.object({
  op: z.string().min(1),
  args: z.record(z.string(), z.unknown()).default({}),
});

export type BridgeRequest = z.infer<typeof BridgeRequestSchema>;

/** Parse a bridge request. */
export function parseBridgeRequest(input: unknown): BridgeRequest {
  return BridgeRequestSchema.parse(input);
}
