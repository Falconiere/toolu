import { z } from "zod";
import { HOSTS } from "../args/types";

/** The only selection-file format version this CLI understands. */
export const SELECTION_VERSION = 1;

export const selectionSchema = z.object({
  version: z.literal(SELECTION_VERSION),
  generatorVersion: z.string().min(1).optional(),
  host: z.enum(HOSTS).optional(),
  enabled: z.array(z.string().min(1)),
});

export type Selection = z.infer<typeof selectionSchema>;

/** The file's own version, read before the full schema so a mismatch reports precisely. */
export const versionProbeSchema = z.object({ version: z.number() });
