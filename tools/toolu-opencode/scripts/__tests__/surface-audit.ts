/**
 * Shared by the generated-surface audits (#358, #355): the committed tree,
 * the Markdown link closure of a set of surfaces, the host-mapping rows, the
 * frontmatter rules the host enforces, and a generator run over a sandbox copy
 * of one plugin with one source edited.
 */
import { cpSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { expect } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { listPluginManifests } from "../../src/inventory/scan.ts";
import { frontmatterName, parseCanonical } from "../../src/surfaces/frontmatter.ts";
import { planSurface } from "../lib/emit.ts";

export const ROOT = resolve(import.meta.dir, "../../../..");
export const GENERATED = join(ROOT, "tools/toolu-opencode/generated");

export const read = (rel: string): string => readFileSync(join(GENERATED, rel), "utf8");

/** `surfaces` plus every Markdown file reachable from them through relative links. */
export function linkedClosure(surfaces: readonly string[]): string[] {
  const seen = new Set<string>();
  const queue = [...surfaces];
  for (let rel = queue.shift(); rel !== undefined; rel = queue.shift()) {
    if (seen.has(rel)) continue;
    seen.add(rel);
    for (const [, target = ""] of read(rel).matchAll(/\]\(([^)#\s]+\.md)(?:#[^)]*)?\)/g)) {
      queue.push(join(dirname(rel), target));
    }
  }
  return [...seen].toSorted();
}

// origin/main's Claude Code and Codex cells, which an OpenCode column must not disturb.
export const HOST_CELLS = [
  "| Invoke a plugin workflow | `/plugin:name` | `$plugin:name` |",
  "| Delegate bounded work | `Agent` / `Task` with an explicit tier | `spawn_agent` with the matching custom agent when installed |",
  "| Ask a structured user choice | `AskUserQuestion` | `request_user_input` when available; otherwise ask one concise question |",
  "| Inspect or steer delegated work | the host's agent controls | Codex subagent thread controls (`/agent` in CLI) |",
  "| Isolate a write-heavy task | `EnterWorktree` / `ExitWorktree` | native `git worktree` commands with an exact, validated path |",
  "| Current external information | `WebSearch` / `WebFetch` or installed research plugins | Codex web access or installed research plugins |",
];

/** The body rows of a generated host-mapping table. */
export function hostMappingRows(rel: string): string[] {
  return read(rel)
    .split("\n")
    .filter(
      (line) => line.startsWith("| ") && !line.startsWith("| Concept") && !line.startsWith("| ---"),
    );
}

/** Every `skill({ name })` id in `text`. */
export const skillNames = (text: string): string[] =>
  [...text.matchAll(/skill\(\{ name: "([^"]+)" \}\)/g)].map(([, id = ""]) => id);

/** The generated skill ids and agent ids. */
export function generatedIds(): { skills: Set<string>; agents: Set<string> } {
  return {
    skills: new Set(readdirSync(join(GENERATED, "skills"))),
    agents: new Set(readdirSync(join(GENERATED, "agents")).map((name) => name.slice(0, -3))),
  };
}

/** Assert the generated skill at `skills/<id>` carries frontmatter the host accepts. */
export function expectHostValidSkill(dir: string): void {
  const text = read(`${dir}/SKILL.md`);
  const parsed = parseCanonical(text);
  if (!parsed.ok) throw new Error(`${dir}: ${parsed.reason}`);
  const id = dir.slice("skills/".length);
  expect(parsed.data.name).toBe(id);
  expect(frontmatterName(text)).toBe(id);
  expect(id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
  expect(id.length).toBeLessThanOrEqual(64);
  const description = parsed.data.description;
  expect(typeof description === "string" && description.length > 0).toBe(true);
  expect(String(description).length).toBeLessThanOrEqual(1024);
}

/**
 * Plan `plugin`'s surface from a sandbox copy in which `file` (relative to the
 * plugin) is rewritten by `edit`; returns a thunk rethrowing what planning threw.
 */
export function planEdited(
  plugin: string,
  file: string,
  edit: (text: string) => string,
): () => unknown {
  using sb = createSandbox();
  const pluginDir = join(sb.root, "plugins", plugin);
  cpSync(join(ROOT, "plugins", plugin), pluginDir, {
    recursive: true,
    filter: (path) => !path.includes("/node_modules"),
  });
  const target = join(pluginDir, file);
  writeFileSync(target, edit(readFileSync(target, "utf8")));
  const manifest = listPluginManifests(join(ROOT, "plugins"))?.find((item) => item.name === plugin);
  if (!manifest) throw new Error(`${plugin} manifest missing`);
  let thrown: unknown;
  try {
    planSurface({
      repoRoot: sb.root,
      outDir: join(sb.root, "generated"),
      plugins: [{ ...manifest, pluginDir }],
    });
  } catch (error) {
    thrown = error;
  }
  return () => {
    throw thrown ?? new Error("planSurface did not throw");
  };
}
