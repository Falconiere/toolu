import { cpSync, existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { listPluginManifests } from "../../src/inventory/scan.ts";
import { frontmatterName, parseCanonical } from "../../src/surfaces/frontmatter.ts";
import { planSurfaces } from "../../src/surfaces/plan.ts";
import { stagePlugins } from "../bundle-plugins.ts";
import { planSurface } from "../lib/emit.ts";

// The toolu and toolu-review surfaces OpenCode loads (#358), audited on the
// committed generated tree: no Claude Code or Codex interface leaks in, and
// every name or path they hand the model resolves.

const ROOT = resolve(import.meta.dir, "../../../..");
const GENERATED = join(ROOT, "tools/toolu-opencode/generated");
const HOST_MAPPING = "resources/toolu/workflows/host-mapping.md";
const AGENTS = [
  "toolu-architect",
  "toolu-deep-explore",
  "toolu-implementer",
  "toolu-quick-task",
  "toolu-research-agent",
];

const read = (rel: string): string => readFileSync(join(GENERATED, rel), "utf8");

function listed(dir: string, keep: (name: string) => boolean): string[] {
  return readdirSync(join(GENERATED, dir))
    .filter(keep)
    .map((name) => `${dir}/${name}`);
}

const SKILL_DIRS = listed("skills", (name) => name.startsWith("toolu-"));
const SURFACES = [
  ...SKILL_DIRS.map((dir) => `${dir}/SKILL.md`),
  ...listed("agents", (name) => name.startsWith("toolu-")),
  ...listed("commands", (name) => name.startsWith("toolu-")),
];

/** The surfaces plus every Markdown file reachable from them through relative links. */
function linkedClosure(): string[] {
  const seen = new Set<string>();
  const queue = [...SURFACES];
  for (let rel = queue.shift(); rel !== undefined; rel = queue.shift()) {
    if (seen.has(rel)) continue;
    seen.add(rel);
    for (const [, target = ""] of read(rel).matchAll(/\]\(([^)#\s]+\.md)(?:#[^)]*)?\)/g)) {
      queue.push(join(dirname(rel), target));
    }
  }
  return [...seen].toSorted();
}

const CLOSURE = linkedClosure();

const BANNED = [
  "ToolSearch",
  "AskUserQuestion",
  "request_user_input",
  "spawn_agent",
  "EnterWorktree",
  "ExitWorktree",
  "mcp__",
  "CODEX_HOME",
  "general-purpose",
  "$toolu:",
  "/toolu:",
  ".claude/tmp",
  ".codex/tmp",
  "Pass `model:`",
  "plugins/toolu/scripts/",
];

// origin/main's Claude Code and Codex cells, which the OpenCode column must not disturb.
const HOST_CELLS = [
  "| Invoke a plugin workflow | `/plugin:name` | `$plugin:name` |",
  "| Delegate bounded work | `Agent` / `Task` with an explicit tier | `spawn_agent` with the matching custom agent when installed |",
  "| Ask a structured user choice | `AskUserQuestion` | `request_user_input` when available; otherwise ask one concise question |",
  "| Inspect or steer delegated work | the host's agent controls | Codex subagent thread controls (`/agent` in CLI) |",
  "| Isolate a write-heavy task | `EnterWorktree` / `ExitWorktree` | native `git worktree` commands with an exact, validated path |",
  "| Current external information | `WebSearch` / `WebFetch` or installed research plugins | Codex web access or installed research plugins |",
];

test("the host mapping adds an OpenCode column and keeps the other cells", () => {
  const rows = read(HOST_MAPPING)
    .split("\n")
    .filter(
      (line) => line.startsWith("| ") && !line.startsWith("| Concept") && !line.startsWith("| ---"),
    );
  expect(rows.length).toBe(HOST_CELLS.length);
  rows.forEach((row, index) => expect(row.startsWith(`${HOST_CELLS[index]} `)).toBe(true));
  const opencode = rows.map((row) => row.split(" | ").at(-1) ?? "").join("\n");
  for (const name of ["`skill`", "`task`", "`subagent_type`", "`question`", "`task_id`"]) {
    expect(opencode).toContain(name);
  }
  expect(opencode).toContain("native `git worktree`");
  expect(opencode).toContain("`websearch` / `webfetch`");
});

test("the audit covers 14 surfaces and the resources they link", () => {
  expect(SURFACES.length).toBe(14);
  expect(CLOSURE).toContain(HOST_MAPPING);
  expect(CLOSURE).toContain("resources/toolu/workflows/semantic-judgments.md");
  expect(CLOSURE).toContain("skills/toolu-orchestrator/references/model-routing.md");
});

test.each(CLOSURE.filter((rel) => rel !== HOST_MAPPING))(
  "%s names no Claude Code or Codex interface",
  (rel) => {
    const text = read(rel);
    expect(BANNED.filter((token) => text.includes(token))).toEqual([]);
  },
);

test("every skill, agent and generated path the surfaces name resolves", () => {
  using sb = createSandbox();
  const staged = join(sb.root, "staged");
  stagePlugins(join(ROOT, "plugins"), staged);
  const skills = new Set(readdirSync(join(GENERATED, "skills")));
  const agents = new Set(readdirSync(join(GENERATED, "agents")).map((name) => name.slice(0, -3)));
  const text = CLOSURE.map(read).join("\n");

  const skillNames = [...text.matchAll(/skill\(\{ name: "([^"]+)" \}\)/g)].map(([, id]) => id);
  expect(skillNames.length).toBeGreaterThan(0);
  expect(skillNames.filter((id) => id === undefined || !skills.has(id))).toEqual([]);

  const quoted = [...text.matchAll(/[`"](toolu-[a-z0-9-]+)[`"]/g)].map(([, id = ""]) => id);
  expect(new Set(quoted)).toContain("toolu-quick-task");
  expect(quoted.filter((id) => !agents.has(id) && !skills.has(id))).toEqual([]);

  const scripts = [...text.matchAll(/\$TOOLU_PLUGIN_ROOT_TOOLU\/([^\s`"]+)/g)].map(
    ([, p = ""]) => p,
  );
  expect(scripts.length).toBeGreaterThan(0);
  expect(scripts.filter((path) => !existsSync(join(staged, "toolu", path)))).toEqual([]);

  const generated = [...text.matchAll(/\$\{TOOLU_OPENCODE_ROOT\}\/generated\/([^\s`")]+)/g)].map(
    ([, p = ""]) => p,
  );
  expect(generated.filter((path) => !existsSync(join(GENERATED, path)))).toEqual([]);
});

test("every surface has frontmatter the host accepts", () => {
  for (const dir of SKILL_DIRS) {
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
  const plan = planSurfaces(GENERATED, ["toolu", "toolu-review"]);
  if (!plan.ok) throw new Error(plan.reason);
  expect(plan.plan.skills.map((skill) => skill.id).toSorted()).toEqual(
    SKILL_DIRS.map((dir) => dir.slice("skills/".length)).toSorted(),
  );
  expect(plan.plan.agents.map((agent) => agent.id).toSorted()).toEqual(AGENTS);
  for (const agent of plan.plan.agents) expect(agent.entry.mode).toBe("subagent");
  expect(plan.plan.commands.length).toBe(2);
  for (const command of plan.plan.commands) {
    const loads = /Load the `([^`]+)` skill/.exec(String(command.entry.template))?.[1] ?? "";
    expect(existsSync(join(GENERATED, "skills", loads, "SKILL.md"))).toBe(true);
  }
});

test.each(AGENTS)("%s states how OpenCode picks its model", (id) => {
  const text = read(`agents/${id}.md`);
  expect(text).toContain(
    `On OpenCode this agent runs \`agent.${id}.model\` from your \`opencode.json\`, else the session's model.`,
  );
  expect(text).not.toContain("This agent runs on");
});

test.each([
  "skills/toolu-orchestrator/SKILL.md",
  "skills/toolu-orchestrator/references/model-routing.md",
  "resources/toolu/skills/orchestrator/references/model-routing.md",
])("%s routes by subagent_type with no model argument", (rel) => {
  const text = read(rel);
  expect(text).toContain("`subagent_type`");
  expect(text).toContain("the `task` tool takes no model argument");
});

test("the orchestrator says subagents do not nest by default", () => {
  expect(read("skills/toolu-orchestrator/SKILL.md")).toContain(
    "OpenCode's default `subagent_depth` of 1 refuses a `task` call made from inside a subagent.",
  );
});

/** Plan the toolu surface from a sandbox copy of the plugin whose debug skill `edit` rewrites. */
function planEditedDebug(edit: (skill: string) => string): () => unknown {
  using sb = createSandbox();
  const pluginDir = join(sb.root, "plugins/toolu");
  cpSync(join(ROOT, "plugins/toolu"), pluginDir, {
    recursive: true,
    filter: (path) => !path.includes("/node_modules"),
  });
  const skill = join(pluginDir, "skills/debug/SKILL.md");
  writeFileSync(skill, edit(readFileSync(skill, "utf8")));
  const manifest = listPluginManifests(join(ROOT, "plugins"))?.find(
    (item) => item.name === "toolu",
  );
  if (!manifest) throw new Error("toolu manifest missing");
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

const SENTRY_ANCHOR = "1. The Sentry MCP's fetch tools only appear **after** OAuth";

test("generation fails naming the source when a port anchor is gone", () => {
  expect(
    planEditedDebug((skill) => skill.replace(SENTRY_ANCHOR, "1. Sentry tools appear after OAuth")),
  ).toThrow(
    `opencode port: plugins/toolu/skills/debug/SKILL.md: expected 1 match(es), found 0: ${SENTRY_ANCHOR}`,
  );
});

test("generation fails naming the source when a port anchor repeats", () => {
  const line = readFileSync(join(ROOT, "plugins/toolu/skills/debug/SKILL.md"), "utf8")
    .split("\n")
    .find((text) => text.startsWith(SENTRY_ANCHOR));
  if (line === undefined) throw new Error("Sentry anchor missing from the source");
  expect(planEditedDebug((skill) => `${skill}\n${line}\n`)).toThrow(
    `opencode port: plugins/toolu/skills/debug/SKILL.md: expected 1 match(es), found 2: ${SENTRY_ANCHOR}`,
  );
});
