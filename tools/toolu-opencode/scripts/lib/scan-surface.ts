/** Scan plugin trees for portable surface sources (#206). */
import { existsSync, readdirSync, readFileSync, realpathSync, lstatSync, statSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import type { PluginManifest } from "../../src/inventory/types.ts";
import type { ArtifactKind } from "./constants.ts";
import { parseFrontmatter, serializeFrontmatter } from "./frontmatter.ts";
import { rewriteBody } from "./rewrite.ts";
import { assignSurfaceIds, candidateKey, type ArtifactCandidate } from "./ids.ts";

export type SurfaceArtifact = {
  kind: ArtifactKind;
  plugin: string;
  surfaceId: string;
  sourcePath: string;
  relativeSource: string;
  content: string;
  strippedKeys: string[];
  rewriteNotes: ReturnType<typeof rewriteBody>["notes"];
  skillDirName?: string;
};

type SourceEntry = {
  kind: ArtifactKind;
  path: string;
  localId: string;
  text: string;
};

function listSkillFiles(skillsRoot: string): string[] {
  if (!existsSync(skillsRoot)) {
    return [];
  }
  const out: string[] = [];
  for (const entry of readdirSync(skillsRoot).sort()) {
    const skillFile = join(skillsRoot, entry, "SKILL.md");
    if (existsSync(skillFile)) {
      out.push(skillFile);
    }
  }
  return out;
}

function listMarkdownFiles(dir: string): string[] {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir)
    .filter((name) => name.endsWith(".md"))
    .sort()
    .map((name) => join(dir, name));
}

function localIdFromParsed(
  kind: ArtifactKind,
  sourcePath: string,
  name: string | undefined,
): string {
  if (name?.trim()) {
    return name.trim();
  }
  if (kind === "skill") {
    return basename(dirname(sourcePath));
  }
  return basename(sourcePath, ".md");
}

function collectSources(root: string): SourceEntry[] {
  const entries: SourceEntry[] = [];
  for (const path of listMarkdownFiles(join(root, "agents"))) {
    const text = readFileSync(path, "utf8");
    const parsed = parseFrontmatter(text);
    entries.push({
      kind: "agent",
      path,
      localId: localIdFromParsed("agent", path, parsed.frontmatter.name),
      text,
    });
  }
  for (const path of listMarkdownFiles(join(root, "commands"))) {
    const text = readFileSync(path, "utf8");
    const parsed = parseFrontmatter(text);
    entries.push({
      kind: "command",
      path,
      localId: localIdFromParsed("command", path, parsed.frontmatter.name),
      text,
    });
  }
  for (const path of listSkillFiles(join(root, "skills"))) {
    const text = readFileSync(path, "utf8");
    const parsed = parseFrontmatter(text);
    entries.push({
      kind: "skill",
      path,
      localId: localIdFromParsed("skill", path, parsed.frontmatter.name),
      text,
    });
  }
  return entries;
}

function buildMarkdown(
  surfaceId: string,
  text: string,
): {
  content: string;
  strippedKeys: string[];
  rewriteNotes: ReturnType<typeof rewriteBody>["notes"];
} {
  const parsed = parseFrontmatter(text);
  const { body, notes } = rewriteBody(parsed.body);
  const frontmatter = { ...parsed.frontmatter, name: surfaceId };
  return {
    content: `${serializeFrontmatter(frontmatter)}${body}`,
    strippedKeys: parsed.strippedKeys,
    rewriteNotes: notes,
  };
}

export function buildSurfaceForPlugin(
  manifest: PluginManifest,
  repoRoot: string,
): SurfaceArtifact[] {
  const plugin = manifest.name;
  const sources = collectSources(manifest.pluginDir);
  const candidates: ArtifactCandidate[] = sources.map((s) => ({
    kind: s.kind,
    plugin,
    localId: s.localId,
  }));
  const idMap = assignSurfaceIds(candidates);
  const artifacts: SurfaceArtifact[] = [];

  for (const source of sources) {
    const surfaceId = idMap.get(
      candidateKey({ kind: source.kind, plugin, localId: source.localId }),
    );
    if (!surfaceId) {
      throw new Error(`missing id for ${source.kind} ${source.localId}`);
    }
    const built = buildMarkdown(surfaceId, source.text);
    artifacts.push({
      kind: source.kind,
      plugin,
      surfaceId,
      sourcePath: source.path,
      relativeSource: relative(repoRoot, source.path),
      ...(source.kind === "skill" ? { skillDirName: basename(dirname(source.path)) } : {}),
      ...built,
    });
  }

  return artifacts.sort((a, b) => a.surfaceId.localeCompare(b.surfaceId));
}

function listFilesRecursive(dir: string, rootReal: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir).sort()) {
    const full = join(dir, name);
    const st = lstatSync(full);
    if (st.isSymbolicLink()) {
      const real = realpathSync(full);
      if (!real.startsWith(rootReal + "/") && real !== rootReal) {
        continue;
      }
    }
    if (st.isDirectory()) {
      out.push(...listFilesRecursive(full, rootReal));
    } else if (st.isFile()) {
      out.push(full);
    }
  }
  return out;
}

/** Add skill `references/` and `scripts/` files into the planned output map. */
export function planSkillResources(
  artifact: SurfaceArtifact,
  outSkillDir: string,
  files: Map<string, string>,
): void {
  if (artifact.kind !== "skill") {
    return;
  }
  const skillDir = dirname(artifact.sourcePath);
  for (const sub of ["references", "scripts"] as const) {
    const src = join(skillDir, sub);
    if (!existsSync(src) || !statSync(src).isDirectory()) {
      continue;
    }
    const srcReal = realpathSync(src);
    for (const file of listFilesRecursive(src, srcReal)) {
      const rel = relative(src, file);
      if (rel.startsWith("..")) {
        continue;
      }
      const dest = join(outSkillDir, sub, rel);
      files.set(dest, readFileSync(file, "utf8"));
    }
  }
}
