/** Write generated OpenCode surface tree (#206). */
import { mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import type { PluginManifest } from "../../src/inventory/types.ts";
import { EXCLUDED_SURFACES, GENERATED_SEGMENT, TOOLU_OPENCODE_ROOT } from "./constants.ts";
import {
  buildSurfaceForPlugin,
  collectSurfaceSources,
  planSkillResources,
} from "./scan-surface.ts";
import { assignSurfaceIds, candidateKey, type ArtifactCandidate } from "./ids.ts";

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
    classification: "generated" | "no-surface";
    reason?: string;
    excluded?: Array<{ source: string; reason: string; owner: string }>;
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
      "OpenCode surface catalog. version=1; surfaceRoot=repo-relative; every plugin is classified; skills|agents|commands[]={id,path,source}; path is relative to surfaceRoot.",
  };
}

function relativeOut(path: string, outDir: string): string {
  return relative(outDir, path).split("\\").join("/");
}

export function planSurface(options: GenerateSurfaceOptions): GenerateSurfaceResult {
  const { repoRoot, outDir } = options;
  const plugins = [...options.plugins].sort((a, b) =>
    a.name < b.name ? -1 : a.name > b.name ? 1 : 0,
  );
  const files = new Map<string, string>();
  const notes: string[] = [];
  const catalogPlugins: GeneratedCatalog["plugins"] = [];

  let totalRewrites = 0;
  let explicitRewrites = 0;
  let sourcePathRewrites = 0;
  let claudeConfigRewrites = 0;
  const dotClaude = new Set<string>();
  const stripped = new Set<string>();

  const sourceEntries = plugins.flatMap((manifest) =>
    collectSurfaceSources(manifest.pluginDir).map((source) => ({ manifest, source })),
  );
  const candidates: ArtifactCandidate[] = sourceEntries.map(({ manifest, source }) => ({
    kind: source.kind,
    plugin: manifest.name,
    localId: source.localId,
  }));
  const idMap = assignSurfaceIds(candidates);
  const invocations = new Map<string, string>();
  const paths = new Map<string, string>();
  for (const candidate of [...candidates].sort((a, b) =>
    a.kind === b.kind ? 0 : a.kind === "skill" ? -1 : 1,
  )) {
    const reference = `${candidate.plugin}:${candidate.localId}`;
    const id = idMap.get(candidateKey(candidate));
    if (id && !invocations.has(reference)) invocations.set(reference, id);
  }
  for (const { manifest, source } of sourceEntries) {
    const id = idMap.get(
      candidateKey({
        kind: source.kind,
        plugin: manifest.name,
        localId: source.localId,
      }),
    );
    if (!id) throw new Error(`missing id for ${source.path}`);
    const sourcePath = relative(repoRoot, source.path).split("\\").join("/");
    const generatedPath =
      source.kind === "skill"
        ? `generated/skills/${id}/SKILL.md`
        : `generated/${source.kind}s/${id}.md`;
    paths.set(sourcePath, `${TOOLU_OPENCODE_ROOT}/${generatedPath}`);
    if (source.kind === "skill") {
      const sourceDir = `plugins/${manifest.name}/skills/${basename(dirname(source.path))}/`;
      paths.set(sourceDir, `${TOOLU_OPENCODE_ROOT}/generated/skills/${id}/`);
    }
  }
  const references = { invocations, paths };

  const artifactsByPlugin = new Map(
    plugins.map((manifest) => [
      manifest.name,
      buildSurfaceForPlugin(manifest, repoRoot, idMap, references),
    ]),
  );
  const sourceToGenerated = new Map<string, string>();
  for (const artifacts of artifactsByPlugin.values()) {
    for (const artifact of artifacts) {
      const path =
        artifact.kind === "skill"
          ? join(outDir, "skills", artifact.surfaceId, "SKILL.md")
          : join(
              outDir,
              artifact.kind === "agent" ? "agents" : "commands",
              `${artifact.surfaceId}.md`,
            );
      sourceToGenerated.set(artifact.sourcePath, path);
    }
  }
  const processedResources = new Set<string>();

  for (const manifest of plugins) {
    const artifacts = artifactsByPlugin.get(manifest.name) ?? [];
    const entry: GeneratedCatalog["plugins"][number] = {
      name: manifest.name,
      classification: artifacts.length ? "generated" : "no-surface",
      ...(artifacts.length
        ? {}
        : { reason: "Plugin provides hooks only; no skill, agent, or command source." }),
      excluded: Object.entries(EXCLUDED_SURFACES)
        .filter(([path]) => path.startsWith(`${manifest.name}/`))
        .map(([path, details]) => ({ source: `plugins/${path}`, ...details })),
      skills: [],
      agents: [],
      commands: [],
    };

    for (const artifact of artifacts) {
      totalRewrites += artifact.rewriteNotes.claudePluginRootRewrites;
      explicitRewrites += artifact.rewriteNotes.explicitReferenceRewrites;
      sourcePathRewrites += artifact.rewriteNotes.sourcePathRewrites;
      claudeConfigRewrites += artifact.rewriteNotes.claudeConfigRewrites;
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
        planSkillResources(
          artifact,
          skillDir,
          outDir,
          repoRoot,
          sourceToGenerated,
          processedResources,
          files,
        );
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
    `The catalog covers all ${catalogPlugins.length} plugin manifests. Runtime installation and enabled-plugin selection are handled separately by OP-11.`,
    "",
    "## Catalog coverage",
    "",
    ...catalogPlugins.map(
      (plugin) =>
        `- \`${plugin.name}\`: ${plugin.classification}; ${plugin.skills.length} skill(s), ${plugin.agents.length} agent(s), ${plugin.commands.length} command(s)${plugin.reason ? ` — ${plugin.reason}` : ""}`,
    ),
    "",
    "## Excluded host-specific surfaces",
    "",
    ...catalogPlugins.flatMap((plugin) =>
      (plugin.excluded ?? []).map(
        (item) => `- \`${item.source}\`: ${item.reason} Owner: ${item.owner}.`,
      ),
    ),
    "",
    "## Path rewrites",
    "",
    `- Claude plugin-root tokens → the owning plugin's \`\${TOOLU_PLUGIN_ROOT_<PLUGIN>}\`: ${totalRewrites}.`,
    `- Claude config-root tokens → OpenCode config root: ${claudeConfigRewrites}.`,
    `- Typed source paths → \`\${TOOLU_OPENCODE_ROOT}/generated/…\` paths: ${sourcePathRewrites}.`,
    `- Explicit skill invocations → generated skill IDs: ${explicitRewrites}.`,
    "",
    "## Runtime environment",
    "",
    "The OpenCode adapter's `shell.env` hook gives every bash call these variables (see `docs/opencode.md`):",
    "",
    "- `TOOLU_PLUGIN_ROOT_<PLUGIN>`: each enabled plugin's directory, the name upper-cased with `-` as `_`.",
    "- `TOOLU_PLUGIN_ROOT`: the toolu core plugin's directory.",
    "- `TOOLU_OPENCODE_ROOT`: the `@toolu/opencode` package directory, which holds `generated/`.",
    "- `TOOLU_CONFIG_DIR`: the project's data root, where helpers such as `context7/search.sh` are published.",
    "- `TOOLU_USER_CONFIG_DIR`, `TOOLU_HOST_OVERRIDE`, `TOOLU_PROJECT_CONFIG_DIRNAME`, `TOOLU_SETTINGS_DIR` and `TOOLU_BUN`; `PATH` gains Bun's directory only when it has no `bun`.",
    "",
    "## Stripped frontmatter",
    "",
    stripped.size > 0
      ? [...stripped]
          .sort()
          .map((k) => `- \`${k}\``)
          .join("\n")
      : "- (none)",
    "",
    "## Literal `.claude` references (not rewritten)",
    "",
    dotClaude.size > 0
      ? [...dotClaude]
          .sort()
          .map((line) => `- ${line}`)
          .join("\n")
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
