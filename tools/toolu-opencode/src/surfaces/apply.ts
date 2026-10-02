/**
 * Apply a surface plan to the host's merged config from the `config` hook (#345).
 *
 * - Skills: each planned skill's directory is appended to `skills.paths`, unless
 *   the host already discovers a skill with that name. The user's copy then stays
 *   the only one, so precedence never depends on parse timing.
 * - Agents and commands: toolu's entry is added, or merged under the user's
 *   entry of the same name (user keys win, as with the host's built-ins).
 * - Shared procedures: generated skills link `generated/resources/`, outside
 *   their own directories. When no user permission rule could match
 *   `external_directory`, toolu allows that one tree, like the host's own
 *   allowance for skill directories; otherwise the user's rules decide.
 *
 * Each config key is computed first and assigned once. A key whose current
 * value is not the shape the host validates is left alone with a note.
 */
import { z } from "zod";
import { isPlainRecord, mergeUnder } from "./merge.ts";
import type { PlannedEntry, SurfacePlan } from "./plan.ts";
import { existingSkillNames, type SkillScanScope } from "./skill-names.ts";

export type SurfaceReport = {
  skills: string[];
  agents: string[];
  commands: string[];
  kept: Array<{ id: string; location: string }>;
  merged: Array<{ kind: "agent" | "command"; id: string }>;
  resourcesAllowed: boolean;
  notes: string[];
};

const SkillsConfig = z.looseObject({ paths: z.array(z.string()).optional() });
const ConfigRecord = z.record(z.string(), z.unknown());

/** The current value of `key` as a record, `{}` when absent, undefined when unusable. */
function recordAt(config: object, key: string): Record<string, unknown> | undefined {
  const raw: unknown = Reflect.get(config, key);
  if (raw === undefined) return {};
  const parsed = ConfigRecord.safeParse(raw);
  return parsed.success ? parsed.data : undefined;
}

function applySkills(
  config: object,
  plan: SurfacePlan,
  scope: Omit<SkillScanScope, "configuredPaths">,
  report: SurfaceReport,
): void {
  if (plan.skills.length === 0) return;
  const raw: unknown = Reflect.get(config, "skills");
  const parsed = SkillsConfig.safeParse(raw ?? {});
  if (!parsed.success) {
    report.notes.push("skills config unreadable; no toolu skills added");
    return;
  }
  const configured = parsed.data.paths ?? [];
  const existing = existingSkillNames({ ...scope, configuredPaths: configured });
  const added: string[] = [];
  for (const skill of plan.skills) {
    const location = existing.get(skill.id);
    if (location === undefined) {
      added.push(skill.dir);
      report.skills.push(skill.id);
    } else report.kept.push({ id: skill.id, location });
  }
  if (added.length > 0)
    Reflect.set(config, "skills", { ...parsed.data, paths: [...configured, ...added] });
}

function applyEntries(
  config: object,
  kind: "agent" | "command",
  planned: readonly PlannedEntry[],
  report: SurfaceReport,
): void {
  if (planned.length === 0) return;
  const current = recordAt(config, kind);
  if (current === undefined) {
    report.notes.push(`${kind} config unreadable; no toolu ${kind}s added`);
    return;
  }
  const next = { ...current };
  for (const { id, entry } of planned) {
    const user = next[id];
    if (user !== undefined && !isPlainRecord(user)) {
      report.notes.push(`${kind} ${id} left as your config defines it`);
      continue;
    }
    next[id] = user === undefined ? structuredClone(entry) : mergeUnder(entry, user);
    if (user !== undefined) report.merged.push({ kind, id });
    report[kind === "agent" ? "agents" : "commands"].push(id);
  }
  Reflect.set(config, kind, next);
}

/** A permission key the host would match against `external_directory`. */
function couldMatchExternal(key: string): boolean {
  return key === "external_directory" || key.includes("*") || key.includes("?");
}

function allowResources(config: object, plan: SurfacePlan, report: SurfaceReport): void {
  if (report.skills.length === 0) return;
  const permission = recordAt(config, "permission");
  if (permission === undefined || Object.keys(permission).some(couldMatchExternal)) {
    report.notes.push(
      "external_directory for toolu's shared procedures left to your permission rules",
    );
    return;
  }
  const pattern = `${plan.generatedDir}/resources/*`;
  Reflect.set(config, "permission", { ...permission, external_directory: { [pattern]: "allow" } });
  report.resourcesAllowed = true;
}

/** Add `plan` to the host config; never throws for any config shape the host accepted. */
export function applySurfaces(
  config: object,
  plan: SurfacePlan,
  scope: Omit<SkillScanScope, "configuredPaths">,
): SurfaceReport {
  const report: SurfaceReport = {
    skills: [],
    agents: [],
    commands: [],
    kept: [],
    merged: [],
    resourcesAllowed: false,
    notes: [],
  };
  applySkills(config, plan, scope, report);
  applyEntries(config, "agent", plan.agents, report);
  applyEntries(config, "command", plan.commands, report);
  allowResources(config, plan, report);
  return report;
}
