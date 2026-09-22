import { z } from "zod";
import type { HostAdapter, HostCommand, InstalledPlugin } from "./types";

const entrySchema = z.object({
  id: z.string(),
  version: z.string(),
  enabled: z.boolean().optional(),
  installPath: z.string().optional(),
});

const listSchema = z.array(entrySchema);

function splitId(id: string): { name: string; marketplace: string } {
  const at = id.lastIndexOf("@");
  if (at <= 0) return { name: id, marketplace: "" };
  return { name: id.slice(0, at), marketplace: id.slice(at + 1) };
}

/** Claude Code's `claude plugin` surface. */
export const claudeAdapter: HostAdapter = {
  host: "claude",
  bin: "claude",
  addMarketplace: (source: string): HostCommand => ({
    argv: ["claude", "plugin", "marketplace", "add", source],
  }),
  install: (name: string, marketplace: string, scope: string | undefined): HostCommand => ({
    argv: [
      "claude",
      "plugin",
      "install",
      `${name}@${marketplace}`,
      ...(scope === undefined ? [] : ["--scope", scope]),
    ],
  }),
  remove: (name: string, marketplace: string): HostCommand => ({
    argv: ["claude", "plugin", "uninstall", `${name}@${marketplace}`],
  }),
  update: (name: string, marketplace: string): HostCommand => ({
    argv: ["claude", "plugin", "update", `${name}@${marketplace}`],
  }),
  listInstalled: (): HostCommand => ({ argv: ["claude", "plugin", "list", "--json"] }),
  listAvailable: (): HostCommand => ({
    argv: ["claude", "plugin", "list", "--available", "--json"],
  }),
  parseList: (stdout: string): readonly InstalledPlugin[] => {
    const parsed = listSchema.safeParse(JSON.parse(stdout));
    if (!parsed.success) throw new Error(`claude plugin list --json: ${parsed.error.message}`);
    return parsed.data.map((entry) => {
      const { name, marketplace } = splitId(entry.id);
      return {
        name,
        marketplace,
        version: entry.version,
        enabled: entry.enabled ?? true,
        path: entry.installPath,
      };
    });
  },
};
