/** Inventory row types for gate coverage (#209). */
import { z } from "zod";

const ClassificationSchema = z.enum(["shell-out", "port-native", "port-new", "no-map"]);
export type Classification = z.infer<typeof ClassificationSchema>;

const SupportSchema = z.enum(["required", "supported", "blocked", "n/a"]);
export type Support = z.infer<typeof SupportSchema>;

const KindSchema = z.enum([
  "hooks.json",
  "concern",
  "pre-tools.d",
  "post-tools.d",
  "session-start.d",
  "builtin-module",
  "entrypoint",
  "lib",
]);
export type Kind = z.infer<typeof KindSchema>;

const DiscoveredSchema = z.object({
  id: z.string(),
  sourcePath: z.string(),
  plugin: z.string(),
  kind: KindSchema,
  event: z.string(),
  matcher: z.string(),
  commandOrModule: z.string(),
  parentId: z.string().nullable(),
});
export type Discovered = z.infer<typeof DiscoveredSchema>;

export const InventoryRowSchema = DiscoveredSchema.extend({
  semantics: z.string(),
  classification: z.string(),
  hostMechanism: z.string(),
  support: z.string(),
  implementationIssue: z.number().nullable(),
  implementationStatus: z.enum(["todo", "wip", "done", "n/a"]),
  verificationBaseline: z.string(),
  verificationConformance: z.string(),
  limits: z.string(),
  bashRequired: z.boolean(),
});
export type InventoryRow = z.infer<typeof InventoryRowSchema>;

export const CLASSIFICATIONS = new Set<string>(ClassificationSchema.options);
