/** Classification enum + precedence placeholder (#210). */
import { z } from "zod";

export const ClassificationSchema = z.enum(["shell-out", "port-native", "port-new", "no-map"]);

export type Classification = z.infer<typeof ClassificationSchema>;

/** Parse a portable-core classification token. */
export function parseClassification(input: unknown): Classification {
  return ClassificationSchema.parse(input);
}
