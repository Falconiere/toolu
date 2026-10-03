/**
 * What toolu contributes for the selected plugins (#345): each plugin's skills,
 * agents and commands from the generated catalog, in catalog order.
 *
 * Agent and command entries are what the host's own Markdown loaders build: the
 * frontmatter plus the trimmed body as `prompt` or `template`. The entry's key
 * supplies the name. The frontmatter must be the generator's canonical form
 * with only the fields the generator emits, so a drifted package fails here,
 * before any hook is returned, instead of reaching the host half-parsed.
 */
import { readFileSync } from "node:fs";
import { dirname } from "node:path";
import { z } from "zod";
import { readSurfaceCatalog, type CatalogEntry } from "./catalog.ts";
import { parseCanonical } from "./frontmatter.ts";

export type PlannedSkill = { id: string; plugin: string; dir: string };
export type PlannedEntry = { id: string; plugin: string; entry: Record<string, unknown> };

export type SurfacePlan = {
  generatedDir: string;
  plugins: string[];
  skills: PlannedSkill[];
  agents: PlannedEntry[];
  commands: PlannedEntry[];
  notes: string[];
};

const Action = z.enum(["allow", "ask", "deny"]);
const EntryKinds = {
  agents: {
    body: "prompt",
    schema: z.strictObject({
      description: z.string().min(1),
      mode: z.enum(["subagent", "primary", "all"]),
      permission: z.record(z.string(), z.union([Action, z.record(z.string(), Action)])).optional(),
      model: z.string().min(1).optional(),
      temperature: z.number().optional(),
      steps: z.number().int().positive().optional(),
      hidden: z.boolean().optional(),
      color: z.string().min(1).optional(),
    }),
  },
  commands: {
    body: "template",
    schema: z.strictObject({
      description: z.string().min(1),
      agent: z.string().min(1).optional(),
      model: z.string().min(1).optional(),
      subtask: z.boolean().optional(),
    }),
  },
} as const;
type EntryKind = keyof typeof EntryKinds;

function plannedEntry(kind: EntryKind, plugin: string, entry: CatalogEntry): PlannedEntry {
  const markdown = parseCanonical(readFileSync(entry.file, "utf8"));
  if (!markdown.ok) throw new Error(`${kind} ${entry.id}: ${markdown.reason}`);
  const { body, schema } = EntryKinds[kind];
  const fields = schema.safeParse(markdown.data);
  if (!fields.success) throw new Error(`${kind} ${entry.id}: ${z.prettifyError(fields.error)}`);
  return { id: entry.id, plugin, entry: { ...fields.data, [body]: markdown.body.trim() } };
}

export type PlanResult = { ok: true; plan: SurfacePlan } | { ok: false; reason: string };

/** The surfaces of `selected` plugins under `generatedDir` (a real path), in catalog order. */
export function planSurfaces(generatedDir: string, selected: readonly string[]): PlanResult {
  const catalog = readSurfaceCatalog(generatedDir);
  if (!catalog.ok) return catalog;
  const wanted = new Set(selected);
  const plan: SurfacePlan = {
    generatedDir,
    plugins: [],
    skills: [],
    agents: [],
    commands: [],
    notes: [],
  };
  try {
    for (const plugin of catalog.plugins) {
      if (!wanted.has(plugin.name)) continue;
      plan.plugins.push(plugin.name);
      for (const skill of plugin.skills)
        plan.skills.push({ id: skill.id, plugin: plugin.name, dir: dirname(skill.file) });
      for (const kind of ["agents", "commands"] as const)
        for (const entry of plugin[kind]) plan[kind].push(plannedEntry(kind, plugin.name, entry));
    }
  } catch (error) {
    return { ok: false, reason: error instanceof Error ? error.message : String(error) };
  }
  for (const name of selected)
    if (!plan.plugins.includes(name)) plan.notes.push(`no generated surface for plugin "${name}"`);
  return { ok: true, plan };
}
