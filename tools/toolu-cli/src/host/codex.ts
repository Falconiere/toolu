import { z } from "zod";
import type { HostAdapter, HostCommand, InstalledPlugin } from "./types";

const entrySchema = z.object({
  name: z.string(),
  marketplaceName: z.string().optional(),
  version: z.string(),
  enabled: z.boolean().optional(),
  source: z.object({ path: z.string().optional() }).optional(),
});

const listSchema = z.object({ installed: z.array(entrySchema) });

/** Codex's `codex plugin` surface. Codex has no installation scope. */
export const codexAdapter: HostAdapter = {
  host: "codex",
  bin: "codex",
  addMarketplace: (source: string): HostCommand => ({
    argv: ["codex", "plugin", "marketplace", "add", source],
  }),
  install: (name: string, marketplace: string): HostCommand => ({
    argv: ["codex", "plugin", "add", `${name}@${marketplace}`],
  }),
  remove: (name: string, marketplace: string): HostCommand => ({
    argv: ["codex", "plugin", "remove", `${name}@${marketplace}`],
  }),
  update: (name: string, marketplace: string): HostCommand => ({
    argv: ["codex", "plugin", "add", `${name}@${marketplace}`],
  }),
  listInstalled: (): HostCommand => ({ argv: ["codex", "plugin", "list", "--json"] }),
  listAvailable: (): HostCommand => ({
    argv: ["codex", "plugin", "list", "--available", "--json"],
  }),
  parseList: (stdout: string): readonly InstalledPlugin[] => {
    const parsed = listSchema.safeParse(JSON.parse(stdout));
    if (!parsed.success) throw new Error(`codex plugin list --json: ${parsed.error.message}`);
    return parsed.data.installed.map((entry) => ({
      name: entry.name,
      marketplace: entry.marketplaceName ?? "",
      version: entry.version,
      enabled: entry.enabled ?? true,
      path: entry.source?.path,
    }));
  },
};
