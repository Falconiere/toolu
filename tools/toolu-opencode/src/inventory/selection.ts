/**
 * Enabled plugin selection (#211, #345).
 *
 * Sources, first present wins:
 * 1. `<project>/.opencode/toolu/plugins.json` — `{ "version": 1, "enabled": ["toolu", ...] }`
 * 2. `<global config root>/toolu/plugins.json`, same schema, when a global root is given.
 * 3. Default: every installed plugin is enabled.
 *
 * An explicit file that cannot be read or fails its schema is an error, never a
 * silent fallback: the user chose a set, and toolu cannot tell which. Names that
 * are not installed are dropped and reported. Then `skills.<pluginName> === false`
 * in `<project>/.opencode/toolu.config.json` disables an otherwise-enabled plugin.
 *
 * Never reads Claude `installed_plugins.json` or Codex install snapshots.
 */
import { existsSync, lstatSync, readFileSync } from "node:fs";
import {
  opencodeGlobalPluginSelectionPath,
  opencodePluginSelectionPath,
  opencodeProjectConfigPath,
} from "../host/roots.ts";
import { listPluginManifests } from "./scan.ts";
import { z } from "zod";
import { TooluConfigSchema } from "@toolu/core/config";

const SelectionFileSchema = z
  .object({
    version: z.literal(1),
    enabled: z.array(z.string().min(1)),
  })
  .strict();

export type SelectionSource = "project" | "global" | "default";

/**
 * An explicit file names its path and the listed names that are not installed;
 * the default (every installed plugin) has neither, so no note can lack a path.
 */
export type EnabledSelection =
  | { ok: true; enabled: Set<string>; source: "default" }
  | {
      ok: true;
      enabled: Set<string>;
      source: "project" | "global";
      path: string;
      unknown: string[];
    }
  | { ok: false; reason: string };

type SelectionFile =
  | { kind: "absent" }
  | { kind: "invalid"; reason: string }
  | { kind: "valid"; enabled: string[] };

/** Only a path that does not exist at all is absent; a dangling link or unreadable path is not. */
function missing(path: string): boolean {
  try {
    lstatSync(path);
    return false;
  } catch (error) {
    return error instanceof Error && "code" in error && error.code === "ENOENT";
  }
}

function readSelectionFile(path: string): SelectionFile {
  let raw: unknown;
  try {
    raw = JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    if (missing(path)) return { kind: "absent" };
    const detail = error instanceof Error ? error.message : String(error);
    return { kind: "invalid", reason: `invalid ${path}: ${detail}` };
  }
  const parsed = SelectionFileSchema.safeParse(raw);
  if (!parsed.success)
    return { kind: "invalid", reason: `invalid ${path}: ${z.prettifyError(parsed.error)}` };
  return { kind: "valid", enabled: parsed.data.enabled };
}

type ReadSelection = Exclude<SelectionFile, { kind: "absent" }>;
type ChosenSelection =
  | { source: "default" }
  | { source: "project" | "global"; path: string; file: ReadSelection };

/** The project file when present, else the global file when a global root is given, else none. */
function chooseSelection(
  projectRoot: string,
  globalConfigRoot: string | undefined,
): ChosenSelection {
  const projectPath = opencodePluginSelectionPath(projectRoot);
  const project = readSelectionFile(projectPath);
  if (project.kind !== "absent") return { source: "project", path: projectPath, file: project };
  if (globalConfigRoot === undefined) return { source: "default" };
  const globalPath = opencodeGlobalPluginSelectionPath(globalConfigRoot);
  const global = readSelectionFile(globalPath);
  if (global.kind !== "absent") return { source: "global", path: globalPath, file: global };
  return { source: "default" };
}

function readSkillsDisabled(projectRoot: string): Set<string> {
  const disabled = new Set<string>();
  const configPath = opencodeProjectConfigPath(projectRoot);
  if (!existsSync(configPath)) {
    return disabled;
  }
  try {
    const raw: unknown = JSON.parse(readFileSync(configPath, "utf8"));
    const parsed = TooluConfigSchema.safeParse(raw);
    if (!parsed.success) {
      return disabled;
    }
    const skills = parsed.data.skills;
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

/** Resolve enabled plugin names for a project + plugins root; `globalConfigRoot` adds the global file. */
export function resolveEnabledPluginNames(
  pluginsRoot: string,
  projectRoot: string,
  globalConfigRoot?: string,
): EnabledSelection {
  const manifests = listPluginManifests(pluginsRoot);
  if (manifests === null) {
    return { ok: false, reason: `cannot read plugins root: ${pluginsRoot}` };
  }
  const installed = new Set(manifests.map((m) => m.name));
  const disabled = readSkillsDisabled(projectRoot);
  const keep = (name: string): boolean => installed.has(name) && !disabled.has(name);
  const chosen = chooseSelection(projectRoot, globalConfigRoot);
  if (chosen.source === "default")
    return { ok: true, enabled: new Set([...installed].filter(keep)), source: "default" };
  const { source, path, file } = chosen;
  if (file.kind === "invalid") return { ok: false, reason: file.reason };
  const enabled = new Set(file.enabled.filter(keep));
  const unknown = [...new Set(file.enabled.filter((name) => !installed.has(name)))];
  return { ok: true, enabled, source, path, unknown };
}
