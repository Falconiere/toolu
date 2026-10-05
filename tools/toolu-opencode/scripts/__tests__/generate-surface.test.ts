import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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
import { TOOLU_OPENCODE_ROOT } from "../lib/constants.ts";
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

test("generated resource links are independent of plugin directory order", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-order-"));
  const manifests = listPluginManifests(join(root, "plugins"));
  if (!manifests) throw new Error("plugin manifests missing");
  const forward = planSurface({ repoRoot: root, outDir: out, plugins: manifests });
  const reverse = planSurface({ repoRoot: root, outDir: out, plugins: [...manifests].reverse() });
  expect(treesEqual(forward.files, reverse.files, out)).toEqual([]);
  const jevDoc = forward.files.get(join(out, "resources/repo/docs/jev/README.md"));
  expect(jevDoc).toContain("../../../../skills/jev-jev/references/problem-solving.md");
  expect(jevDoc).toContain("../../../../skills/jev-jev/evals/README.md");
});

test("agent-browser skill renders one project-scoped OpenCode helper command", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-browser-surface-"));
  const browser = listPluginManifests(join(root, "plugins"))?.find(
    (plugin) => plugin.name === "agent-browser",
  );
  if (browser === undefined) throw new Error("agent-browser manifest missing");
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: [browser] });
  const skill = plan.files.get(join(out, "skills/agent-browser-agent-browser/SKILL.md"));
  expect(skill).toBeDefined();
  expect(skill).toContain("# OpenCode");
  expect(skill).toContain('"${TOOLU_CONFIG_DIR}/agent-browser/agent-browser.sh"');
  expect(skill).not.toContain("# Codex");
  expect(skill).not.toContain("# Claude Code");
  expect(skill).toContain("snapshot");
  expect(skill).toContain("--content-boundaries");
  expect(skill).toContain("agent-browser install");
  expect(skill).toContain("context7-context7");
  expect(skill).toContain("exa-search-exa-search");
  expect(skill).toContain("when enabled");
  rmSync(out, { recursive: true, force: true });
});

test("exa-search skill renders project helper commands and a no-key fallback", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-exa-surface-"));
  const exa = listPluginManifests(join(root, "plugins"))?.find(
    (plugin) => plugin.name === "exa-search",
  );
  if (exa === undefined) throw new Error("exa-search manifest missing");
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: [exa] });
  const skill = plan.files.get(join(out, "skills/exa-search-exa-search/SKILL.md"));
  expect(skill).toContain("# OpenCode");
  expect(skill).toContain('"${TOOLU_CONFIG_DIR}/exa-search/search.sh" search');
  expect(skill).toContain('"${TOOLU_CONFIG_DIR}/exa-search/search.sh" crawl');
  expect(skill).toContain('"${TOOLU_CONFIG_DIR}/exa-search/search.sh" similar');
  expect(skill).toContain("EXA_API_KEY");
  expect(skill).toContain("websearch");
  expect(skill).toContain("webfetch");
  expect(skill).not.toContain("# Codex");
  expect(skill).not.toContain("# Claude Code");
  rmSync(out, { recursive: true, force: true });
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
  const [first, second] = candidates;
  if (!first || !second) throw new Error("candidates missing");
  expect(forward.get(candidateKey(first))).not.toBe(forward.get(candidateKey(second)));
  expect(() => assignSurfaceIds([first, first])).toThrow();
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
  const result = rewriteBody(source, references, { plugin: "brainstorm" });
  expect(result.body).toBe('Use `skill({ name: "delivery-flow-delivery-flow" })` here.');
  expect(result.notes.explicitReferenceRewrites).toBe(2);
});

test("each plugin's root token becomes that plugin's own root variable", () => {
  const references = { invocations: new Map<string, string>(), paths: new Map<string, string>() };
  const source = 'ROOT="${CLAUDE_PLUGIN_ROOT}"\nS="${CLAUDE_PLUGIN_ROOT}/scripts"';
  const result = rewriteBody(source, references, { plugin: "epic-orchestrator" });
  expect(result.body).toBe(
    'ROOT="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR}"\nS="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR}/scripts"',
  );
  expect(result.notes.claudePluginRootRewrites).toBe(2);
});

test("no generated file names the ambiguous braced TOOLU_PLUGIN_ROOT, and the notes state the env contract", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-roots-"));
  const root = repoRoot();
  const plugins = listPluginManifests(join(root, "plugins")) ?? [];
  const plan = planSurface({ repoRoot: root, outDir: out, plugins });
  const braced = [...plan.files].filter(([, text]) => text.includes("${TOOLU_PLUGIN_ROOT}"));
  expect(braced.map(([path]) => path)).toEqual([]);
  const epic = plan.files.get(join(out, "skills/epic-orchestrator-epic-orchestrator/SKILL.md"));
  expect(epic).toContain('ROOT="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?');
  expect(epic).not.toContain("${PLUGIN_ROOT");
  const notes = plan.files.get(join(out, "GENERATED-NOTES.md")) ?? "";
  expect(notes).toContain("`TOOLU_PLUGIN_ROOT_<PLUGIN>`");
  expect(notes).toContain("`TOOLU_OPENCODE_ROOT`");
  expect(notes).not.toContain("package root.\n- The OpenCode adapter must set `TOOLU_PLUGIN_ROOT`");
});

test("generated research agent uses native OpenCode web tools", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-agent-"));
  const files = planDefault(out).files;
  const agent = files.get(join(out, "agents/toolu-research-agent.md"));
  const skill = files.get(join(out, "skills/toolu-deep-research/SKILL.md"));
  expect(agent).toContain("`websearch` and `webfetch`");
  expect(agent).not.toContain("search.sh");
  expect(skill).toContain("native web search and fetch");
  expect(skill).not.toContain("search.sh");
});

test("generated delivery skill loads brainstorm by its generated ID", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-delivery-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), [
    "delivery-flow",
    "brainstorm",
  ]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  const skill = plan.files.get(join(out, "skills/delivery-flow-delivery-flow/SKILL.md"));
  expect(skill).toContain('`skill({ name: "brainstorm-brainstorm" })`');
  expect(skill).not.toContain("brainstorm:brainstorm");
});

test("generated review skill labels its OpenCode config example", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-review-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["toolu-review"]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  const skill = plan.files.get(join(out, "skills/toolu-review-review/SKILL.md"));
  expect(skill).toContain("# OpenCode\n");
  expect(skill).toContain("/opencode}/toolu-review/write-state.sh");
  expect(skill).toContain("TOOLU_HOST_OVERRIDE=opencode");
  expect(skill).not.toContain("${CLAUDE_CONFIG_DIR:-$HOME/.claude}");
});

test("generated status skill runs the selected plugin's report under shell.env", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-status-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["statusline"]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  const skill = plan.files.get(join(out, "skills/statusline-status/SKILL.md")) ?? "";
  expect(skill).toContain(
    '# OpenCode\n"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"\n',
  );
  expect(skill).toContain("or toolu plugin readiness status in OpenCode.");
  expect(skill).toContain("OpenCode has no persistent statusline");
  for (const stale of ["../../", "Codex", "TOOLU_HOST_OVERRIDE=codex", "bun ../"]) {
    expect(skill).not.toContain(stale);
  }
  expect(existsSync(join(root, "plugins/statusline/hooks/dist/status.js"))).toBe(true);
  const statusline = plan.catalog.plugins.find((plugin) => plugin.name === "statusline");
  expect(statusline?.excluded).toEqual([
    {
      source: "plugins/statusline/commands/setup.md",
      reason:
        "Claude Code statusLine setting in settings.json; OpenCode has no statusline setting, so the persistent statusline is host-specific. Use the statusline-status skill.",
      owner: "OP-25 (#359)",
    },
  ]);
  expect(statusline?.commands).toEqual([]);
});

test("generated Jev skill and its examples name only the OpenCode wrapper and ignore .env", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-jev-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["jev"]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  const wrapper =
    '# OpenCode\nJEV="${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jev/jev.sh"\n';
  for (const file of ["SKILL.md", "references/problem-solving.md"]) {
    const text = plan.files.get(join(out, "skills/jev-jev", file)) ?? "";
    expect(text).toContain(wrapper);
    expect(text).not.toContain("CODEX_HOME");
    expect(text).not.toContain("Claude Code");
    expect(text).toContain('"$JEV_BUN" --no-env-file "$JEV"');
    expect(text).not.toContain('"$JEV_BUN" "$JEV"');
  }
});

test("generated model-routing references resolve relative to each skill", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-routing-"));
  const plan = planDefault(out);
  const references: ReadonlyArray<readonly [string, string]> = [
    ["toolu-deep-research", "../toolu-orchestrator/references/model-routing.md"],
    ["toolu-orchestrator", "references/model-routing.md"],
  ];
  for (const [id, relativePath] of references) {
    const skill = plan.files.get(join(out, "skills", id, "SKILL.md"));
    expect(skill).toContain(`\`${relativePath}\``);
    expect(skill).not.toContain(
      "${TOOLU_OPENCODE_ROOT}/generated/skills/toolu-orchestrator/references/model-routing.md",
    );
    expect(plan.files.has(resolve(out, "skills", id, relativePath))).toBe(true);
  }
});

test("real agent tools map to exact OpenCode permissions and reject unknown tools", () => {
  const sourcePath = join(repoRoot(), "plugins/toolu/agents/architect.md");
  const source = readFileSync(sourcePath, "utf8");
  const references = { invocations: new Map<string, string>(), paths: new Map<string, string>() };
  const rendered = renderMarkdown(
    "agent",
    "toolu-architect",
    sourcePath,
    source,
    references,
    "toolu",
  );
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
  expect(() =>
    renderMarkdown("agent", "toolu-architect", sourcePath, invalid, references, "toolu"),
  ).toThrow("unsupported Claude agent tool: UnknownTool");
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
    `${TOOLU_OPENCODE_ROOT}/generated/skills/${orchestratorSkill?.id}/references/model-routing.md`,
  );
  const context7 = plan.catalog.plugins.find((plugin) => plugin.name === "context7")?.skills[0];
  const context7Skill = plan.files.get(join(out, context7?.path ?? "")) ?? "";
  expect(context7Skill).not.toContain("CLAUDE_CONFIG_DIR");
  // One OpenCode command under shell.env's Bun, which ignores the project .env (#348).
  expect(context7Skill).toContain(
    '# OpenCode\n"$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/context7/search.sh" <command> [options]\n```',
  );
  for (const gone of ["CODEX_HOME", "# Codex", "# Claude Code", "Choose the line"]) {
    expect(context7Skill).not.toContain(gone);
  }
  const jira = plan.catalog.plugins.find((plugin) => plugin.name === "jira")?.skills[0];
  const jiraSkill = plan.files.get(join(out, jira?.path ?? "")) ?? "";
  // One OpenCode command under shell.env's Bun and OpenCode's own state paths (#351).
  expect(jiraSkill).toContain(
    '# OpenCode\n"$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jira/jira.sh" [--api-version N] [--lean] <family> <action> [options]\n```',
  );
  expect(jiraSkill).toContain("`.opencode/tmp/jira/plans/<KEY>.md`.");
  expect(jiraSkill).toContain("below `<repo>/.opencode/tmp/plan-ledger/`.");
  expect(jiraSkill).toContain("loading this skill or reading an issue never authorizes a write");
  for (const gone of [
    "CODEX_HOME",
    "TOOLU_HOST_OVERRIDE=claude",
    "# Codex",
    "# Claude Code",
    "Choose the complete command",
    ".claude/tmp",
    ".codex/tmp",
  ]) {
    expect(jiraSkill).not.toContain(gone);
  }
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
      const target = (match[1] ?? "").split("#", 1)[0];
      if (!target || target.includes("://")) continue;
      expect(plan.files.has(resolve(dirname(file), target))).toBe(true);
    }
  }
  expect([...plan.files.keys()].some((path) => path.endsWith("resources/jev/README.md"))).toBe(
    true,
  );
  const jevReference = plan.files.get(join(out, "skills/jev-jev/references/problem-solving.md"));
  expect(jevReference).toContain("# OpenCode\nJEV=");
  expect(jevReference).toContain("${XDG_CONFIG_HOME:-$HOME/.config}/opencode");
  expect(jevReference).not.toContain("${CLAUDE_CONFIG_DIR:-$HOME/.claude}");
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

test("present directory linked as a skill resource reports a non-file error", () => {
  const root = repoRoot();
  const copyRoot = mkdtempSync(join(tmpBase, "toolu-surface-directory-"));
  const pluginDir = join(copyRoot, "plugins/jev");
  cpSync(join(root, "plugins/jev"), pluginDir, { recursive: true });
  rmSync(join(pluginDir, "README.md"));
  mkdirSync(join(pluginDir, "README.md"));
  const manifest = listPluginManifests(join(root, "plugins"))?.find((item) => item.name === "jev");
  if (!manifest) throw new Error("jev manifest missing");
  expect(() =>
    planSurface({
      repoRoot: copyRoot,
      outDir: join(copyRoot, "generated"),
      plugins: [{ ...manifest, pluginDir }],
    }),
  ).toThrow("linked resource is not a file: ../../README.md");
  rmSync(copyRoot, { recursive: true, force: true });
});
