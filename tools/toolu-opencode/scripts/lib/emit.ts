/** Write generated OpenCode surface tree (#206). */
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import type { PluginManifest } from "../../src/inventory/types.ts";
import { GENERATED_SEGMENT, TOOLU_PLUGIN_ROOT } from "./constants.ts";
import { buildSurfaceForPlugin, planSkillResources } from "./scan-surface.ts";

export type GenerateSurfaceOptions = {
  repoRoot: string;
  outDir: string;
  plugins: PluginManifest[];
};

export type GeneratedCatalog = {
  version: 1;
  surfaceRoot: string;
  plugins: Array<{
    name: string;
    skills: Array<{ id: string; path: string; source: string }>;
    agents: Array<{ id: string; path: string; source: string }>;
    commands: Array<{ id: string; path: string; source: string }>;
  }>;
};

export type GenerateSurfaceResult = {
  files: Map<string, string>;
  catalog: GeneratedCatalog;
  notes: string[];
};

function catalogHeader(): Record<string, string> {
  return {
    $comment:
      "OpenCode toolu surface catalog. version=1; surfaceRoot=repo-relative; plugins[].skills|agents|commands[]={id,path,source}; path is relative to surfaceRoot.",
  };
}

function relativeOut(path: string, outDir: string): string {
  return relative(outDir, path).split("\\").join("/");
}

export function planSurface(options: GenerateSurfaceOptions): GenerateSurfaceResult {
  const { repoRoot, outDir, plugins } = options;
  const files = new Map<string, string>();
  const notes: string[] = [];
  const catalogPlugins: GeneratedCatalog["plugins"] = [];

  let totalRewrites = 0;
  const dotClaude = new Set<string>();
  const stripped = new Set<string>();

  for (const manifest of plugins) {
    const artifacts = buildSurfaceForPlugin(manifest, repoRoot);
    const entry: GeneratedCatalog["plugins"][number] = {
      name: manifest.name,
      skills: [],
      agents: [],
      commands: [],
    };

    for (const artifact of artifacts) {
      totalRewrites += artifact.rewriteNotes.claudePluginRootRewrites;
      for (const line of artifact.rewriteNotes.dotClaudeRefs) {
        dotClaude.add(line);
      }
      for (const key of artifact.strippedKeys) {
        stripped.add(key);
      }

      const ref = {
        id: artifact.surfaceId,
        path: "",
        source: artifact.relativeSource,
      };

      if (artifact.kind === "skill") {
        const skillDir = join(outDir, "skills", artifact.surfaceId);
        const skillFile = join(skillDir, "SKILL.md");
        ref.path = relativeOut(skillFile, outDir);
        files.set(skillFile, artifact.content);
        planSkillResources(artifact, skillDir, files);
        entry.skills.push(ref);
      } else if (artifact.kind === "agent") {
        const agentFile = join(outDir, "agents", `${artifact.surfaceId}.md`);
        ref.path = relativeOut(agentFile, outDir);
        files.set(agentFile, artifact.content);
        entry.agents.push(ref);
      } else {
        const commandFile = join(outDir, "commands", `${artifact.surfaceId}.md`);
        ref.path = relativeOut(commandFile, outDir);
        files.set(commandFile, artifact.content);
        entry.commands.push(ref);
      }
    }

    entry.skills.sort((a, b) => a.id.localeCompare(b.id));
    entry.agents.sort((a, b) => a.id.localeCompare(b.id));
    entry.commands.sort((a, b) => a.id.localeCompare(b.id));
    catalogPlugins.push(entry);
  }

  catalogPlugins.sort((a, b) => a.name.localeCompare(b.name));

  const notesBody = [
    "# Generated surface notes",
    "",
    "Do not edit by hand. Regenerate with `bun run generate:opencode-surface`.",
    "",
    "## Path rewrites",
    "",
    `- \`${TOOLU_PLUGIN_ROOT}\` replaces Claude \`\${CLAUDE_PLUGIN_ROOT}\` (${totalRewrites} substitution(s) in phase-1 \`toolu\` set).`,
    "- Bootstrap must set `TOOLU_PLUGIN_ROOT` to the installed plugin directory (workflows, hooks).",
    "",
    "## Stripped frontmatter",
    "",
    stripped.size > 0
      ? [...stripped].sort().map((k) => `- \`${k}\``).join("\n")
      : "- (none)",
    "",
    "## Literal `.claude` references (not rewritten)",
    "",
    dotClaude.size > 0
      ? [...dotClaude].sort().map((line) => `- ${line}`).join("\n")
      : "- (none in scanned bodies)",
    "",
  ].join("\n");

  const catalog: GeneratedCatalog & { $comment?: string } = {
    ...catalogHeader(),
    version: 1,
    surfaceRoot: GENERATED_SEGMENT,
    plugins: catalogPlugins,
  };

  const catalogPath = join(outDir, "opencode.toolu.json");
  files.set(catalogPath, `${JSON.stringify(catalog, null, 2)}\n`);
  files.set(join(outDir, "GENERATED-NOTES.md"), notesBody);

  notes.push(...notesBody.split("\n"));

  return { files, catalog, notes };
}

export function writeSurface(plan: GenerateSurfaceResult, outDir: string): void {
  const paths = [...plan.files.keys()].sort();
  for (const path of paths) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, plan.files.get(path) ?? "", "utf8");
  }
  pruneStale(outDir, new Set(paths));
}

function pruneStale(outDir: string, keepFiles: Set<string>): void {
  if (!keepFiles.size) {
    return;
  }
  const walk = (dir: string): void => {
    if (!statSync(dir, { throwIfNoEntry: false })?.isDirectory()) {
      return;
    }
    for (const name of readdirSync(dir).sort()) {
      const full = join(dir, name);
      if (statSync(full).isDirectory()) {
        walk(full);
        if (readdirSync(full).length === 0) {
          rmSync(full, { recursive: true });
        }
        continue;
      }
      if (!keepFiles.has(full)) {
        rmSync(full);
      }
    }
  };
  walk(outDir);
}

export function readTree(dir: string): Map<string, string> {
  const out = new Map<string, string>();
  const walk = (current: string): void => {
    if (!statSync(current, { throwIfNoEntry: false })?.isDirectory()) {
      return;
    }
    for (const name of readdirSync(current).sort()) {
      const full = join(current, name);
      if (statSync(full).isDirectory()) {
        walk(full);
      } else {
        out.set(full, readFileSync(full, "utf8"));
      }
    }
  };
  walk(dir);
  return out;
}

export function treesEqual(
  expected: Map<string, string>,
  actual: Map<string, string>,
  outDir: string,
): string[] {
  const diffs: string[] = [];
  const rel = (p: string): string => relative(outDir, p);
  const keys = new Set([...expected.keys(), ...actual.keys()]);
  for (const key of [...keys].sort()) {
    if (!expected.has(key)) {
      diffs.push(`unexpected file: ${rel(key)}`);
      continue;
    }
    if (!actual.has(key)) {
      diffs.push(`missing file: ${rel(key)}`);
      continue;
    }
    if (expected.get(key) !== actual.get(key)) {
      diffs.push(`content differs: ${rel(key)}`);
    }
  }
  return diffs;
}
