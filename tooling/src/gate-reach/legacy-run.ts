/**
 * Stale legacy exemptions. An exemption is an exact path in the owning tool's
 * own config: an oxlint override that only switches rules off, a jscpd ignore
 * entry, a knip workspace ignore entry. Each tool is re-run with its
 * exemptions lifted; an exempted file that is gone, or no longer has the
 * finding it is exempted for, is reported. A tool with no exemption is not run.
 */
import { existsSync } from "node:fs";
import { join, relative } from "node:path";
import { lintDirs } from "../lint-ts.ts";
import { findingKey, jscpdClones, knipFindings, oxlintFindings } from "./legacy-tools.ts";
import { exactPath, isExactPath } from "./reach-files.ts";
import { JscpdSchema, KnipSchema, OxlintSchema, readJson } from "./reach-schema.ts";

type Override = { files: string[]; rules: Record<string, unknown> };

function stale(config: string, file: string, why: string): string {
  return `legacy-exemptions: ${config}: ${file} ${why}; remove the exemption`;
}

/** One exemption of a tool that reports per file: gone, still needed, or stale. */
function fileVerdict(
  root: string,
  config: string,
  file: string,
  found: ReadonlySet<string>,
  why: string,
): string[] {
  if (!existsSync(join(root, file))) return [stale(config, file, "does not exist")];
  return found.has(file) ? [] : [stale(config, file, why)];
}

/** An override that only switches rules off, for files named exactly. */
function isLegacy(override: Override): boolean {
  return (
    override.files.every(isExactPath) &&
    Object.values(override.rules).every((level) => level === "off")
  );
}

function oxlintStale(root: string): string[] {
  return lintDirs(root).flatMap(({ dir, targets }) => {
    const rel = `${relative(root, dir)}/.oxlintrc.json`;
    const config = readJson(root, rel, OxlintSchema);
    const legacy = config.overrides.filter(isLegacy);
    if (legacy.length === 0) return [];
    const lifted = { ...config, overrides: config.overrides.filter((o) => !isLegacy(o)) };
    const found = oxlintFindings(root, dir, targets, lifted);
    return legacy.flatMap(({ files, rules }) =>
      files.flatMap((file) => {
        if (!existsSync(join(dir, file))) return [stale(rel, file, "does not exist")];
        return Object.keys(rules)
          .filter((rule) => !found.has(findingKey(file, rule)))
          .map((rule) => stale(rel, file, `no longer violates ${rule}`));
      }),
    );
  });
}

function jscpdStale(root: string): string[] {
  const config = readJson(root, ".jscpd.json", JscpdSchema);
  const legacy = config.ignore.filter(isExactPath).map(exactPath);
  if (legacy.length === 0) return [];
  const lifted = { ...config, ignore: config.ignore.filter((glob) => !isExactPath(glob)) };
  const clones = jscpdClones(root, lifted);
  return legacy.flatMap((file) => fileVerdict(root, ".jscpd.json", file, clones, "has no clone"));
}

function knipStale(root: string): string[] {
  const config = readJson(root, "knip.json", KnipSchema);
  const workspaces = Object.entries(config.workspaces);
  const legacy = workspaces.flatMap(([dir, { ignore }]) =>
    ignore.filter(isExactPath).map((entry) => (dir === "." ? entry : `${dir}/${entry}`)),
  );
  if (legacy.length === 0) return [];
  const lifted = {
    ...config,
    workspaces: Object.fromEntries(
      workspaces.map(([dir, workspace]) => [
        dir,
        { ...workspace, ignore: workspace.ignore.filter((glob) => !isExactPath(glob)) },
      ]),
    ),
  };
  const found = knipFindings(root, lifted);
  return legacy.flatMap((file) =>
    fileVerdict(root, "knip.json", file, found, "has no dead export"),
  );
}

export function checkLegacy(root: string): string[] {
  return [...oxlintStale(root), ...jscpdStale(root), ...knipStale(root)];
}
