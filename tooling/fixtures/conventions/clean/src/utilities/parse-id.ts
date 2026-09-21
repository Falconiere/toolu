/** Parse a non-empty id string with Zod. */
import { z } from "zod";

const IdSchema = z.string().min(1);

/** Validate unknown input as an id. */
export function parseId(input: unknown): string {
  return IdSchema.parse(input);
}
