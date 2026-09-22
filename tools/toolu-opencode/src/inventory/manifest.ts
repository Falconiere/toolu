/** Read plugin.json manifests from a plugins root (#211). */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import type { PluginManifest } from "./types.ts";

const DependencySchema = z
  .object({
    name: z.string().min(1),
    marketplace: z.string().min(1),
  })
  .strict();

const PluginJsonSchema = z.object({
  name: z.string().min(1),
  version: z.string().min(1),
  dependencies: z.array(DependencySchema).optional(),
});

export function pluginSpec(name: string, marketplace: string): string {
  return `${name}@${marketplace}`;
}

export function readPluginManifest(pluginDir: string): PluginManifest | null {
  const manifestPath = join(pluginDir, ".claude-plugin", "plugin.json");
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch {
    return null;
  }
  const parsed = PluginJsonSchema.safeParse(raw);
  if (!parsed.success) {
    return null;
  }
  const marketplace = "toolu";
  const { name, version, dependencies } = parsed.data;
  return {
    name,
    spec: pluginSpec(name, marketplace),
    marketplace,
    version,
    pluginDir,
    dependencies: dependencies ?? [],
  };
}
