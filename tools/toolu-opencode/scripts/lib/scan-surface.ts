/** Scan plugin trees for portable surface sources (#206). */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative } from "node:path";
import type { PluginManifest } from "../../src/inventory/types.ts";
import type { ArtifactKind } from "./constants.ts";
import { parseFrontmatter, serializeFrontmatter } from "./frontmatter.ts";
import { rewriteBody } from "./rewrite.ts";
import {
  assignSurfaceIds,
  candidateKey,
  type ArtifactCandidate,
} from "./ids.ts";

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

function localIdFromSource(sourcePath: string, kind: ArtifactKind): string {
  const text = readFileSync(sourcePath, "utf8");
  const parsed = parseFrontmatter(text);
  if (parsed.frontmatter.name?.trim()) {
    return parsed.frontmatter.name.trim();
  }
  if (kind === "skill") {
    return basename(join(sourcePath, ".."));
  }
  return basename(sourcePath, ".md");
}

function buildMarkdown(surfaceId: string, sourcePath: string): {
  content: string;
  strippedKeys: string[];
  rewriteNotes: ReturnType<typeof rewriteBody>["notes"];
} {
  const parsed = parseFrontmatter(readFileSync(sourcePath, "utf8"));
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
  const root = manifest.pluginDir;
  const candidates: ArtifactCandidate[] = [];

  for (const agentPath of listMarkdownFiles(join(root, "agents"))) {
    candidates.push({
      kind: "agent",
      plugin,
      localId: localIdFromSource(agentPath, "agent"),
    });
  }
  for (const commandPath of listMarkdownFiles(join(root, "commands"))) {
    candidates.push({
      kind: "command",
      plugin,
      localId: localIdFromSource(commandPath, "command"),
    });
  }
  for (const skillPath of listSkillFiles(join(root, "skills"))) {
    candidates.push({
      kind: "skill",
      plugin,
      localId: localIdFromSource(skillPath, "skill"),
    });
  }

  const idMap = assignSurfaceIds(candidates);
  const artifacts: SurfaceArtifact[] = [];

  for (const agentPath of listMarkdownFiles(join(root, "agents"))) {
    const localId = localIdFromSource(agentPath, "agent");
    const surfaceId = idMap.get(candidateKey({ kind: "agent", plugin, localId }));
    if (!surfaceId) {
      throw new Error(`missing id for agent ${localId}`);
    }
    const built = buildMarkdown(surfaceId, agentPath);
    artifacts.push({
      kind: "agent",
      plugin,
      surfaceId,
      sourcePath: agentPath,
      relativeSource: relative(repoRoot, agentPath),
      ...built,
    });
  }

  for (const commandPath of listMarkdownFiles(join(root, "commands"))) {
    const localId = localIdFromSource(commandPath, "command");
    const surfaceId = idMap.get(candidateKey({ kind: "command", plugin, localId }));
    if (!surfaceId) {
      throw new Error(`missing id for command ${localId}`);
    }
    const built = buildMarkdown(surfaceId, commandPath);
    artifacts.push({
      kind: "command",
      plugin,
      surfaceId,
      sourcePath: commandPath,
      relativeSource: relative(repoRoot, commandPath),
      ...built,
    });
  }

  for (const skillPath of listSkillFiles(join(root, "skills"))) {
    const localId = localIdFromSource(skillPath, "skill");
    const surfaceId = idMap.get(candidateKey({ kind: "skill", plugin, localId }));
    if (!surfaceId) {
      throw new Error(`missing id for skill ${localId}`);
    }
    const built = buildMarkdown(surfaceId, skillPath);
    artifacts.push({
      kind: "skill",
      plugin,
      surfaceId,
      sourcePath: skillPath,
      relativeSource: relative(repoRoot, skillPath),
      skillDirName: basename(join(skillPath, "..")),
      ...built,
    });
  }

  return artifacts.sort((a, b) => a.surfaceId.localeCompare(b.surfaceId));
}

function listFilesRecursive(dir: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir).sort()) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      out.push(...listFilesRecursive(full));
    } else {
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
  const skillDir = join(artifact.sourcePath, "..");
  for (const sub of ["references", "scripts"] as const) {
    const src = join(skillDir, sub);
    if (!existsSync(src) || !statSync(src).isDirectory()) {
      continue;
    }
    for (const file of listFilesRecursive(src)) {
      const rel = relative(src, file);
      const dest = join(outSkillDir, sub, rel);
      files.set(dest, readFileSync(file, "utf8"));
    }
  }
}
