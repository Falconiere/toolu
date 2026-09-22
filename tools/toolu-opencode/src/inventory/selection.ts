/**
 * Enabled plugin selection (#211).
 *
 * Sources (in order):
 * 1. `<project>/.opencode/toolu/plugins.json` — `{ "version": 1, "enabled": ["toolu", ...] }`
 * 2. `<project>/.opencode/toolu.config.json` or repo `.opencode/toolu.config.json` —
 *    `skills.<pluginName> === false` disables an otherwise-installed plugin.
 * 3. Default: every installed plugin is enabled.
 *
 * Never reads Claude `installed_plugins.json` or Codex install snapshots.
 */
import { existsSync, readFileSync } from "node:fs";
import { opencodePluginSelectionPath, opencodeProjectConfigPath } from "../host/roots.ts";
import { listPluginManifests } from "./scan.ts";
import { z } from "zod";
import { parseTooluConfig } from "@toolu/core/config";

const SelectionFileSchema = z
  .object({
    version: z.literal(1),
    enabled: z.array(z.string().min(1)),
  })
  .strict();

function readSelectionFile(path: string): string[] | null {
  if (!existsSync(path)) {
    return null;
  }
  try {
    const raw: unknown = JSON.parse(readFileSync(path, "utf8"));
    const parsed = SelectionFileSchema.safeParse(raw);
    if (!parsed.success) {
      return null;
    }
    return parsed.data.enabled;
  } catch {
    return null;
  }
}

function readSkillsDisabled(projectRoot: string): Set<string> {
  const disabled = new Set<string>();
  const configPath = opencodeProjectConfigPath(projectRoot);
  if (!existsSync(configPath)) {
    return disabled;
  }
  try {
    const raw: unknown = JSON.parse(readFileSync(configPath, "utf8"));
    const config = parseTooluConfig(raw);
    const skills = config.skills;
    if (!skills) {
      return disabled;
    }
    for (const [name, on] of Object.entries(skills)) {
      if (!on) {
        disabled.add(name);
      }
    }
  } catch {
    return disabled;
  }
  return disabled;
}

/** Resolve enabled plugin names for a project + plugins root. */
export function resolveEnabledPluginNames(
  pluginsRoot: string,
  projectRoot: string,
): { ok: true; enabled: Set<string> } | { ok: false; reason: string } {
  const manifests = listPluginManifests(pluginsRoot);
  if (manifests === null) {
    return { ok: false, reason: `cannot read plugins root: ${pluginsRoot}` };
  }
  const installed = new Set(manifests.map((m) => m.name));

  const fromFile = readSelectionFile(opencodePluginSelectionPath(projectRoot));
  let enabled: Set<string>;
  if (fromFile !== null) {
    enabled = new Set(fromFile.filter((name) => installed.has(name)));
  } else {
    enabled = new Set(installed);
  }

  for (const name of readSkillsDisabled(projectRoot)) {
    enabled.delete(name);
  }

  return { ok: true, enabled };
}
