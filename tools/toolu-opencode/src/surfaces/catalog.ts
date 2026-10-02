/**
 * The generated OpenCode surface catalog (#344), validated for installation (#345).
 *
 * `generated/opencode.toolu.json` lists each plugin's skills, agents and
 * commands. Paths resolve against the generated directory; the catalog's
 * repository-relative `surfaceRoot` is ignored. Every entry must name a regular
 * file inside that directory, IDs follow the documented skill-name rule and are
 * unique per kind, and a skill lives at `skills/<id>/SKILL.md` with that `name`,
 * so a user copy found by name always shadows exactly toolu's skill.
 */
import { readFileSync, realpathSync, statSync } from "node:fs";
import { isAbsolute, join, relative } from "node:path";
import { z } from "zod";
import { frontmatterName } from "./frontmatter.ts";

export const SURFACE_KINDS = ["skills", "agents", "commands"] as const;
export type SurfaceKind = (typeof SURFACE_KINDS)[number];

export type CatalogEntry = { id: string; file: string };
export type CatalogPlugin = { name: string } & Record<SurfaceKind, CatalogEntry[]>;
export type Catalog = { ok: true; plugins: CatalogPlugin[] } | { ok: false; reason: string };

export const CATALOG_FILE = "opencode.toolu.json";
const SURFACE_ID = /^[a-z0-9]+(-[a-z0-9]+)*$/;

const Entry = z.object({
  id: z.string().max(64).regex(SURFACE_ID, "an ID is lowercase words joined by single hyphens"),
  path: z.string().min(1),
  source: z.string(),
});
const CatalogSchema = z.looseObject({
  version: z.literal(1),
  plugins: z.array(
    z.looseObject({
      name: z.string().min(1),
      skills: z.array(Entry),
      agents: z.array(Entry),
      commands: z.array(Entry),
    }),
  ),
});
type RawEntry = z.infer<typeof Entry>;

/**
 * The file an entry names, inside `generatedDir`; throws the reason when it is not
 * usable. IDs already match the ID rule; a path is quoted (JSON-escaped), so no
 * catalog string can put control characters into the host log or a refusal.
 */
function entryFile(generatedDir: string, kind: SurfaceKind, entry: RawEntry): string {
  const file = join(generatedDir, entry.path);
  const rel = relative(generatedDir, file);
  const shown = JSON.stringify(entry.path);
  if (rel === "" || rel.startsWith("..") || isAbsolute(rel))
    throw new Error(`${kind} ${entry.id}: path escapes the generated directory: ${shown}`);
  if (kind === "skills" && entry.path !== `skills/${entry.id}/SKILL.md`)
    throw new Error(`skills ${entry.id}: expected skills/${entry.id}/SKILL.md, got ${shown}`);
  let regular = false;
  try {
    regular = statSync(file).isFile();
  } catch {
    regular = false;
  }
  if (!regular) throw new Error(`${kind} ${entry.id}: missing file ${shown}`);
  if (kind === "skills" && frontmatterName(readFileSync(file, "utf8")) !== entry.id)
    throw new Error(`skills ${entry.id}: SKILL.md name is not ${entry.id}`);
  return file;
}

function readJson(path: string): unknown {
  return JSON.parse(readFileSync(path, "utf8"));
}

/** Read and validate the catalog under `generatedDir` (already a real path). */
export function readSurfaceCatalog(generatedDir: string): Catalog {
  const path = join(generatedDir, CATALOG_FILE);
  try {
    const parsed = CatalogSchema.safeParse(readJson(path));
    if (!parsed.success)
      return { ok: false, reason: `invalid ${path}: ${z.prettifyError(parsed.error)}` };
    const seen = new Set<string>();
    const plugins = parsed.data.plugins.map((plugin) => {
      const out: CatalogPlugin = { name: plugin.name, skills: [], agents: [], commands: [] };
      for (const kind of SURFACE_KINDS) {
        for (const entry of plugin[kind]) {
          if (seen.has(`${kind}/${entry.id}`)) throw new Error(`${kind} ${entry.id}: duplicate ID`);
          seen.add(`${kind}/${entry.id}`);
          out[kind].push({ id: entry.id, file: entryFile(generatedDir, kind, entry) });
        }
      }
      return out;
    });
    return { ok: true, plugins };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { ok: false, reason: `catalog ${path}: ${detail}` };
  }
}

/** `<package root>/generated` as a real path, so host locations and permission patterns agree. */
export function realGeneratedDir(packageRoot: string): string | undefined {
  try {
    return realpathSync(join(packageRoot, "generated"));
  } catch {
    return undefined;
  }
}
