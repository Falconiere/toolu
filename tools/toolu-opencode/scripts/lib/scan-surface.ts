/** Scan plugin trees for portable surface sources (#206). */
import {
  existsSync,
  readdirSync,
  readFileSync,
  realpathSync,
  lstatSync,
  statSync,
} from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import type { PluginManifest } from "../../src/inventory/types.ts";
import {
  COMMAND_SKILL_TARGETS,
  EXCLUDED_SURFACES,
  type ArtifactKind,
} from "./constants.ts";
import { parseFrontmatter } from "./frontmatter.ts";
import type { SurfaceReferences } from "./rewrite.ts";
import { candidateKey } from "./ids.ts";
import { renderMarkdown } from "./render.ts";

export type SurfaceArtifact = {
  kind: ArtifactKind;
  plugin: string;
  surfaceId: string;
  sourcePath: string;
  relativeSource: string;
  content: string;
  strippedKeys: string[];
  rewriteNotes: ReturnType<typeof renderMarkdown>["rewriteNotes"];
  skillDirName?: string;
};

export type SourceEntry = {
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
  name: unknown,
): string {
  if (typeof name === "string" && name.trim()) {
    return name.trim();
  }
  if (kind === "skill") {
    return basename(dirname(sourcePath));
  }
  return basename(sourcePath, ".md");
}

export function collectSurfaceSources(root: string): SourceEntry[] {
  const entries: SourceEntry[] = [];
  const plugin = basename(root);
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
    const exclusionKey = `${plugin}/commands/${basename(path)}`;
    if (exclusionKey in EXCLUDED_SURFACES) continue;
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

export function buildSurfaceForPlugin(
  manifest: PluginManifest,
  repoRoot: string,
  idMap: ReadonlyMap<string, string>,
  references: SurfaceReferences,
): SurfaceArtifact[] {
  const plugin = manifest.name;
  const sources = collectSurfaceSources(manifest.pluginDir);
  const artifacts: SurfaceArtifact[] = [];

  for (const source of sources) {
    const surfaceId = idMap.get(
      candidateKey({ kind: source.kind, plugin, localId: source.localId }),
    );
    if (!surfaceId) {
      throw new Error(`missing id for ${source.kind} ${source.localId}`);
    }
    const target =
      COMMAND_SKILL_TARGETS[`${plugin}:${source.localId}`] ??
      `${plugin}:${source.localId}`;
    const [targetPlugin = plugin, targetLocalId = source.localId] =
      target.split(":", 2);
    const commandSkillId =
      source.kind === "command"
        ? idMap.get(
            candidateKey({
              kind: "skill",
              plugin: targetPlugin,
              localId: targetLocalId,
            }),
          )
        : undefined;
    if (
      source.kind === "command" &&
      COMMAND_SKILL_TARGETS[`${plugin}:${source.localId}`] &&
      !commandSkillId
    ) {
      throw new Error(`${source.path}: mapped skill is missing: ${target}`);
    }
    const built = renderMarkdown(
      source.kind,
      surfaceId,
      source.path,
      source.text,
      references,
      plugin,
      commandSkillId,
    );
    artifacts.push({
      kind: source.kind,
      plugin,
      surfaceId,
      sourcePath: source.path,
      relativeSource: relative(repoRoot, source.path),
      ...(source.kind === "skill"
        ? { skillDirName: basename(dirname(source.path)) }
        : {}),
      ...built,
    });
  }

  return artifacts.sort((a, b) => a.surfaceId.localeCompare(b.surfaceId));
}

function listFilesRecursive(dir: string, rootReal: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir).sort()) {
    if (name === "__tests__") continue;
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

function within(root: string, target: string): boolean {
  const rel = relative(root, target);
  return (
    rel === "" ||
    (rel !== ".." && !rel.startsWith("../") && !rel.startsWith("..\\"))
  );
}

function copiedMarkdown(source: string): string {
  const copied = source.replace(
    /[ \t]+(?=\r?$)/gm,
    (spaces: string, offset: number) => {
      const lineStart = source.lastIndexOf("\n", offset - 1) + 1;
      return source.slice(lineStart, offset).trim() && spaces.length >= 2
        ? "\\"
        : "";
    },
  );
  return copied.replace(
    '# Codex (for Claude Code use the second line instead):\nJEV="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/jev/jev.sh"\n# JEV="${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}/jev/jev.sh"',
    '# Codex (for OpenCode use the second line instead):\nJEV="${TOOLU_CONFIG_DIR:-${CODEX_HOME:-$HOME/.codex}}/jev/jev.sh"\n# JEV="${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jev/jev.sh"',
  );
}

/** Copy the local link closure for one generated skill. */
export function planSkillResources(
  artifact: SurfaceArtifact,
  outSkillDir: string,
  outDir: string,
  repoRoot: string,
  sourceToGenerated: ReadonlyMap<string, string>,
  processed: Set<string>,
  files: Map<string, string>,
): void {
  if (artifact.kind !== "skill") return;
  const skillDir = dirname(artifact.sourcePath);
  const skillReal = realpathSync(skillDir);
  const queue: Array<{ source: string; dest: string }> = [
    { source: artifact.sourcePath, dest: join(outSkillDir, "SKILL.md") },
  ];
  for (const file of listFilesRecursive(skillDir, skillReal)) {
    if (file === artifact.sourcePath) continue;
    const dest = join(outSkillDir, relative(skillDir, file));
    files.set(
      dest,
      file.endsWith(".md")
        ? copiedMarkdown(readFileSync(file, "utf8"))
        : readFileSync(file, "utf8"),
    );
    if (file.endsWith(".md")) queue.push({ source: file, dest });
  }
  for (let i = 0; i < queue.length; i += 1) {
    const item = queue[i];
    if (!item || processed.has(item.dest)) continue;
    processed.add(item.dest);
    const content = files.get(item.dest);
    if (content === undefined)
      throw new Error(`missing generated resource: ${item.dest}`);
    const rewritten = content.replace(
      /\]\(([^)]+)\)/g,
      (whole, link: string) => {
        if (link.startsWith("#") || /^[a-z][a-z0-9+.-]*:/i.test(link))
          return whole;
        const [path, fragment] = link.split("#", 2);
        if (!path) return whole;
        const target = join(dirname(item.source), path);
        if (!existsSync(target)) {
          throw new Error(`${item.source}: missing linked resource ${link}`);
        }
        if (!statSync(target).isFile()) {
          throw new Error(`${item.source}: linked resource is not a file: ${link}`);
        }
        const real = realpathSync(target);
        if (!within(repoRoot, real))
          throw new Error(
            `${item.source}: linked resource escapes repo: ${link}`,
          );
        const repoRelative = relative(repoRoot, real);
        const resourceRelative = repoRelative.startsWith("plugins/")
          ? repoRelative.slice("plugins/".length)
          : join("repo", repoRelative);
        const mapped = sourceToGenerated.get(real);
        const dest =
          mapped ??
          (within(skillDir, real)
            ? join(outSkillDir, relative(skillDir, real))
            : join(outDir, "resources", resourceRelative));
        if (!mapped && !files.has(dest)) {
          files.set(
            dest,
            real.endsWith(".md")
              ? copiedMarkdown(readFileSync(real, "utf8"))
              : readFileSync(real, "utf8"),
          );
          if (real.endsWith(".md")) queue.push({ source: real, dest });
        }
        const relativeLink = relative(dirname(item.dest), dest)
          .split("\\")
          .join("/");
        return `](${relativeLink}${fragment ? `#${fragment}` : ""})`;
      },
    );
    files.set(item.dest, rewritten);
  }
}
