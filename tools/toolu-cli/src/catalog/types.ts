import { z } from "zod";

const dependencySchema = z.object({ name: z.string(), marketplace: z.string().optional() });

const catalogEntrySchema = z.object({
  name: z.string().min(1),
  description: z.string().optional(),
  dependencies: z.array(dependencySchema).optional(),
});

export const marketplaceSchema = z.object({
  name: z.string().min(1),
  plugins: z.array(catalogEntrySchema).min(1),
});

export type CatalogEntry = z.infer<typeof catalogEntrySchema>;
export type Marketplace = z.infer<typeof marketplaceSchema>;
