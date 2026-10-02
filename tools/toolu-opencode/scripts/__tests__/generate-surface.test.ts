import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { expect, test } from "bun:test";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { assignSurfaceIds, candidateKey, type ArtifactCandidate } from "../lib/ids.ts";
import { parseFrontmatter, serializeFrontmatter } from "../lib/frontmatter.ts";
import { planSurface, readTree, treesEqual, writeSurface } from "../lib/emit.ts";
import { selectPluginsByEnabledNames } from "../../src/select/resolve.ts";
import { runGenerateSurface } from "../generate-surface.ts";
import { listPluginManifests } from "../../src/inventory/scan.ts";
import { TOOLU_PLUGIN_ROOT } from "../lib/constants.ts";
import { renderMarkdown } from "../lib/render.ts";
import { rewriteBody } from "../lib/rewrite.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../..");
}

function planDefault(outDir: string) {
  const root = repoRoot();
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["toolu"]);
  if (!selected.ok) {
    throw new Error(selected.reason);
  }
  return planSurface({ repoRoot: root, outDir, plugins: selected.plugins });
}

test("generate twice yields identical tree", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-"));
  const plan = planDefault(out);
  writeSurface(plan, out);
  const first = readTree(out);
  writeSurface(plan, out);
  const second = readTree(out);
  expect(treesEqual(first, second, out)).toEqual([]);
});

test("drift check fails when a source skill changes", () => {
  const root = repoRoot();
  const copyRoot = mkdtempSync(join(tmpBase, "toolu-surface-src-"));
  cpSync(join(root, "plugins"), join(copyRoot, "plugins"), { recursive: true });
  cpSync(join(root, "docs"), join(copyRoot, "docs"), { recursive: true });
  cpSync(join(root, "README.md"), join(copyRoot, "README.md"));
  cpSync(join(root, "LICENSE"), join(copyRoot, "LICENSE"));
  cpSync(join(root, "tooling/conventions"), join(copyRoot, "tooling/conventions"), {
    recursive: true,
  });
  const outDir = mkdtempSync(join(tmpBase, "toolu-surface-out-"));
  const code = runGenerateSurface(["--repo", copyRoot, "--out", outDir]);
  expect(code).toBe(0);

  const skillPath = join(copyRoot, "plugins/delivery-flow/skills/delivery-flow/SKILL.md");
  writeFileSync(skillPath, `${readFileSync(skillPath, "utf8")}\n<!-- drift probe -->\n`);
  const drift = runGenerateSurface(["--repo", copyRoot, "--out", outDir, "--check"]);
  expect(drift).toBe(1);
  rmSync(copyRoot, { recursive: true, force: true });
  rmSync(outDir, { recursive: true, force: true });
});

test("toolu surface ids are unique", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-ids-"));
  const plan = planDefault(out);
  const ids: string[] = [];
  for (const plugin of plan.catalog.plugins) {
    for (const item of [...plugin.skills, ...plugin.agents, ...plugin.commands]) {
      ids.push(item.id);
    }
  }
  expect(new Set(ids).size).toBe(ids.length);
});

test("generated skill resources exclude colocated test files", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-resources-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["delivery-flow"]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  expect([...plan.files.keys()].some((path) => path.includes("/__tests__/"))).toBe(false);
});

test("commands with matching skills load the generated skill", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-rewrite-"));
  const plan = planDefault(out);
  const commandRef = plan.catalog.plugins
    .find((plugin) => plugin.name === "toolu")
    ?.commands.find((item) => item.source.endsWith("/commit.md"));
  const skillRef = plan.catalog.plugins
    .find((plugin) => plugin.name === "toolu")
    ?.skills.find((item) => item.source.endsWith("/commit/SKILL.md"));
  const command = commandRef && plan.files.get(join(out, commandRef.path));
  expect(command).toBeDefined();
  expect(command).toContain(`\`${skillRef?.id}\` skill`);
  expect(command).not.toContain("${CLAUDE_PLUGIN_ROOT}");
});

test("committed generated tree passes drift check", () => {
  const root = repoRoot();
  const generated = join(root, "tools/toolu-opencode/generated");
  expect(existsSync(generated)).toBe(true);
  const code = runGenerateSurface(["--repo", root, "--check"]);
  expect(code).toBe(0);
  const manifests = listPluginManifests(join(root, "plugins"));
  if (!manifests) throw new Error("plugin manifests missing");
  const plan = planSurface({ repoRoot: root, outDir: generated, plugins: manifests });
  const git = spawnSync("git", ["ls-files", "-z", "--", "tools/toolu-opencode/generated"], {
    cwd: root,
    encoding: "utf8",
  });
  if (git.status !== 0) throw new Error(`git ls-files failed: ${git.stderr}`);
  const tracked = new Set(git.stdout.split("\0").filter(Boolean));
  for (const path of plan.files.keys()) {
    expect(tracked.has(path.slice(root.length + 1))).toBe(true);
  }
});

test("surface ids obey skill naming rules and remain stable across collisions and length limits", () => {
  const candidates: ArtifactCandidate[] = [
    { kind: "skill", plugin: "a-b", localId: "c" },
    { kind: "skill", plugin: "a", localId: "b-c" },
    { kind: "command", plugin: "a-b", localId: "c" },
    { kind: "skill", plugin: "a-b", localId: "X".repeat(100) },
  ];
  const forward = assignSurfaceIds(candidates);
  const reverse = assignSurfaceIds([...candidates].reverse());
  expect([...forward]).toEqual([...reverse]);
  const ids = [...forward.values()];
  expect(new Set(ids).size).toBe(ids.length);
  for (const id of ids) {
    expect(id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
    expect(id.length).toBeLessThanOrEqual(64);
  }
  expect(forward.get(candidateKey(candidates[0]))).not.toBe(
    forward.get(candidateKey(candidates[1])),
  );
  expect(() => assignSurfaceIds([candidates[0], candidates[0]])).toThrow();
  const hashed: ArtifactCandidate = { kind: "command", plugin: "a", localId: "b" };
  const sameBase: ArtifactCandidate = { kind: "skill", plugin: "a", localId: "b" };
  const suffix = createHash("sha256").update(candidateKey(hashed)).digest("hex").slice(0, 8);
  const reserved: ArtifactCandidate = { kind: "skill", plugin: "a", localId: `b-${suffix}` };
  const reservedIds = assignSurfaceIds([hashed, sameBase, reserved]);
  expect(new Set(reservedIds.values()).size).toBe(3);
  expect(reservedIds.get(candidateKey(reserved))).toBe(`a-b-${suffix}`);
});

test("frontmatter round trip preserves folded text and structured permission maps", () => {
  const source = `---\nname: example\ndescription: >-\n  First line\n  second line\npermission:\n  bash:\n    "*": ask\n    "git diff": allow\nmetadata:\n  audience: maintainers\n---\nBody`;
  const parsed = parseFrontmatter(source);
  expect(parsed.frontmatter.description).toBe("First line second line");
  expect(parsed.frontmatter.permission).toEqual({ bash: { "*": "ask", "git diff": "allow" } });
  expect(
    parseFrontmatter(`${serializeFrontmatter(parsed.frontmatter)}${parsed.body}`).frontmatter,
  ).toEqual(parsed.frontmatter);
  expect(() => parseFrontmatter("---\nname: [broken\n---\nBody")).toThrow();
});

test("real epic skill description keeps its embedded issue number", () => {
  const source = readFileSync(
    join(repoRoot(), "plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md"),
    "utf8",
  );
  const description = parseFrontmatter(source).frontmatter.description;
  expect(description).toContain("work epic #248");
  expect(description).toContain("Not for a single standalone issue or PR.");
});

test("plain YAML descriptions keep a hash after a literal backslash and quote", () => {
  const source = '---\ndescription: Work \\"issue #248\\" today\n---\nBody';
  expect(parseFrontmatter(source).frontmatter.description).toBe('Work \\"issue #248\\" today');
});

test("serializer reports the key for values JSON cannot encode", () => {
  expect(() => serializeFrontmatter({ metadata: { id: 1n } })).toThrow(
    "unsupported frontmatter value: metadata",
  );
});

test("paired host spellings become one OpenCode skill invocation", () => {
  const references = {
    invocations: new Map([["delivery-flow:delivery-flow", "delivery-flow-delivery-flow"]]),
    paths: new Map<string, string>(),
  };
  const source = "Use `/delivery-flow:delivery-flow` or `$delivery-flow:delivery-flow` here.";
  const result = rewriteBody(source, references);
  expect(result.body).toBe('Use `skill({ name: "delivery-flow-delivery-flow" })` here.');
  expect(result.notes.explicitReferenceRewrites).toBe(2);
});

test("real agent tools map to exact OpenCode permissions and reject unknown tools", () => {
  const sourcePath = join(repoRoot(), "plugins/toolu/agents/architect.md");
  const source = readFileSync(sourcePath, "utf8");
  const references = { invocations: new Map<string, string>(), paths: new Map<string, string>() };
  const rendered = renderMarkdown("agent", "toolu-architect", sourcePath, source, references);
  const parsed = parseFrontmatter(rendered.content);
  expect(parsed.frontmatter.mode).toBe("subagent");
  expect(parsed.frontmatter.model).toBeUndefined();
  expect(parsed.frontmatter.permission).toEqual({
    "*": "deny",
    read: "allow",
    grep: "allow",
    glob: "allow",
    bash: "allow",
  });

  const invalid = source.replace("tools: Read, Grep, Glob, Bash", "tools: Read, UnknownTool");
  expect(invalid).not.toBe(source);
  expect(() => renderMarkdown("agent", "toolu-architect", sourcePath, invalid, references)).toThrow(
    "unsupported Claude agent tool: UnknownTool",
  );
});

test("full catalog includes all plugins and explicitly classifies empty surfaces", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-all-"));
  const manifests = listPluginManifests(join(root, "plugins"));
  if (!manifests) throw new Error("plugin manifests missing");
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: manifests });
  expect(plan.catalog.plugins).toHaveLength(16);
  expect(
    plan.catalog.plugins.filter((plugin) => plugin.classification === "no-surface"),
  ).toHaveLength(3);
  expect(plan.catalog.plugins.flatMap((plugin) => plugin.skills)).toHaveLength(18);
  expect(plan.catalog.plugins.flatMap((plugin) => plugin.agents)).toHaveLength(5);
  expect(plan.catalog.plugins.flatMap((plugin) => plugin.commands)).toHaveLength(4);
  expect(plan.catalog.plugins.find((plugin) => plugin.name === "statusline")?.excluded).toEqual([
    expect.objectContaining({ source: "plugins/statusline/commands/setup.md" }),
  ]);
  const epicCommand = plan.catalog.plugins.find((plugin) => plugin.name === "epic-orchestrator")
    ?.commands[0];
  const epicSkill = plan.catalog.plugins.find((plugin) => plugin.name === "epic-orchestrator")
    ?.skills[0];
  expect(plan.files.get(join(out, epicCommand?.path ?? ""))).toContain(
    `\`${epicSkill?.id}\` skill`,
  );
  for (const skill of plan.catalog.plugins.flatMap((plugin) => plugin.skills)) {
    const parsed = parseFrontmatter(plan.files.get(join(out, skill.path)) ?? "");
    expect(skill.id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/);
    expect(skill.id.length).toBeLessThanOrEqual(64);
    expect(dirname(skill.path).split("/").at(-1)).toBe(skill.id);
    expect(parsed.frontmatter.name).toBe(skill.id);
    const description = parsed.frontmatter.description;
    if (typeof description !== "string") throw new Error(`missing description: ${skill.id}`);
    expect(description.length).toBeGreaterThan(0);
  }
  for (const agent of plan.catalog.plugins.flatMap((plugin) => plugin.agents)) {
    const parsed = parseFrontmatter(plan.files.get(join(out, agent.path)) ?? "");
    expect(parsed.frontmatter.mode).toBe("subagent");
    expect(parsed.frontmatter.model).toBeUndefined();
    expect(parsed.frontmatter.tools).toBeUndefined();
    expect(parsed.frontmatter.permission).toEqual(expect.objectContaining({ "*": "deny" }));
  }
  for (const command of plan.catalog.plugins.flatMap((plugin) => plugin.commands)) {
    const parsed = parseFrontmatter(plan.files.get(join(out, command.path)) ?? "");
    expect(typeof parsed.frontmatter.description).toBe("string");
  }
  const brainstorm = plan.catalog.plugins.find((plugin) => plugin.name === "brainstorm")?.skills[0];
  expect(plan.files.get(join(out, brainstorm?.path ?? ""))).toContain(
    "delivery-flow-delivery-flow",
  );
  const orchestratorSkill = plan.catalog.plugins
    .find((plugin) => plugin.name === "toolu")
    ?.skills.find((skill) => skill.source.endsWith("/orchestrator/SKILL.md"));
  const deepExplore = plan.catalog.plugins
    .find((plugin) => plugin.name === "toolu")
    ?.agents.find((agent) => agent.source.endsWith("/deep-explore.md"));
  expect(plan.files.get(join(out, deepExplore?.path ?? ""))).toContain(
    `${TOOLU_PLUGIN_ROOT}/generated/skills/${orchestratorSkill?.id}/references/model-routing.md`,
  );
  const context7 = plan.catalog.plugins.find((plugin) => plugin.name === "context7")?.skills[0];
  expect(plan.files.get(join(out, context7?.path ?? ""))).not.toContain("CLAUDE_CONFIG_DIR");
});

test("resource links in generated skills resolve inside the output tree", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-links-"));
  const manifests = listPluginManifests(join(root, "plugins"));
  if (!manifests) throw new Error("plugin manifests missing");
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: manifests });
  for (const [file, content] of plan.files) {
    if (!file.endsWith(".md")) continue;
    for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
      const target = match[1].split("#", 1)[0];
      if (!target || target.includes("://")) continue;
      expect(plan.files.has(resolve(dirname(file), target))).toBe(true);
    }
  }
  expect([...plan.files.keys()].some((path) => path.endsWith("resources/jev/README.md"))).toBe(
    true,
  );
});

test("missing real skill resource fails generation with its source path", () => {
  const root = repoRoot();
  const copyRoot = mkdtempSync(join(tmpBase, "toolu-surface-missing-"));
  const pluginDir = join(copyRoot, "plugins/jev");
  cpSync(join(root, "plugins/jev"), pluginDir, { recursive: true });
  rmSync(join(pluginDir, "README.md"));
  const manifest = listPluginManifests(join(root, "plugins"))?.find((item) => item.name === "jev");
  if (!manifest) throw new Error("jev manifest missing");
  expect(() =>
    planSurface({
      repoRoot: copyRoot,
      outDir: join(copyRoot, "generated"),
      plugins: [{ ...manifest, pluginDir }],
    }),
  ).toThrow("missing linked resource ../../README.md");
  rmSync(copyRoot, { recursive: true, force: true });
});
