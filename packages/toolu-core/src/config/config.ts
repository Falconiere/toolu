/** toolu.config.json Zod placeholder (version: 1) (#210). */
import { z } from "zod";

export const TooluConfigSchema = z
  .object({
    version: z.literal(1),
  })
  .strict();

export type TooluConfig = z.infer<typeof TooluConfigSchema>;

/** Parse toolu.config.json. */
export function parseTooluConfig(input: unknown): TooluConfig {
  return TooluConfigSchema.parse(input);
}
