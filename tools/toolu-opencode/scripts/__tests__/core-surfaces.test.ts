import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { planSurfaces } from "../../src/surfaces/plan.ts";
import { stagePlugins } from "../bundle-plugins.ts";
import {
  GENERATED,
  HOST_CELLS,
  ROOT,
  expectHostValidSkill,
  generatedIds,
  hostMappingRows,
  linkedClosure,
  planEdited,
  read,
  skillNames,
} from "./surface-audit.ts";

// The toolu and toolu-review surfaces OpenCode loads (#358), audited on the
// committed generated tree: no Claude Code or Codex interface leaks in, and
// every name or path they hand the model resolves.

const HOST_MAPPING = "resources/toolu/workflows/host-mapping.md";
const AGENTS = [
  "toolu-architect",
  "toolu-deep-explore",
  "toolu-implementer",
  "toolu-quick-task",
  "toolu-research-agent",
];

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

const CLOSURE = linkedClosure(SURFACES);

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

test("the host mapping adds an OpenCode column and keeps the other cells", () => {
  const rows = hostMappingRows(HOST_MAPPING);
  expect(rows.length).toBe(HOST_CELLS.length);
  rows.forEach((row, index) => expect(row.startsWith(`${HOST_CELLS[index]} `)).toBe(true));
  const opencode = rows.map((row) => row.split(" | ").at(-1) ?? "").join("\n");
  for (const name of ["`skill`", "`task`", "`subagent_type`", "`question`", "`task_id`"]) {
    expect(opencode).toContain(name);
  }
  expect(opencode).toContain("native `git worktree`");
  expect(opencode).toContain("`websearch` / `webfetch`");
});

test("the audit covers 16 surfaces and the resources they link", () => {
  expect(SURFACES.length).toBe(16);
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
  const { skills, agents } = generatedIds();
  const text = CLOSURE.map(read).join("\n");

  const named = skillNames(text);
  expect(named.length).toBeGreaterThan(0);
  expect(named.filter((id) => !skills.has(id))).toEqual([]);

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
  for (const dir of SKILL_DIRS) expectHostValidSkill(dir);
  const plan = planSurfaces(GENERATED, ["toolu", "toolu-review"]);
  if (!plan.ok) throw new Error(plan.reason);
  expect(plan.plan.skills.map((skill) => skill.id).toSorted()).toEqual(
    SKILL_DIRS.map((dir) => dir.slice("skills/".length)).toSorted(),
  );
  expect(plan.plan.agents.map((agent) => agent.id).toSorted()).toEqual(AGENTS);
  for (const agent of plan.plan.agents) expect(agent.entry.mode).toBe("subagent");
  expect(plan.plan.commands.length).toBe(4);
  const loading = plan.plan.commands.filter((command) =>
    /Load the `[^`]+` skill/.test(String(command.entry.template)),
  );
  expect(loading).toHaveLength(2);
  for (const command of loading) {
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

const DEBUG = "skills/debug/SKILL.md";
const SENTRY_ANCHOR = "1. The Sentry MCP's fetch tools only appear **after** OAuth";

test("generation fails naming the source when a port anchor is gone", () => {
  expect(
    planEdited("toolu", DEBUG, (skill) =>
      skill.replace(SENTRY_ANCHOR, "1. Sentry tools appear after OAuth"),
    ),
  ).toThrow(
    `opencode port: plugins/toolu/skills/debug/SKILL.md: expected 1 match(es), found 0: ${SENTRY_ANCHOR}`,
  );
});

test("generation fails naming the source when a port anchor repeats", () => {
  const line = readFileSync(join(ROOT, "plugins/toolu", DEBUG), "utf8")
    .split("\n")
    .find((text) => text.startsWith(SENTRY_ANCHOR));
  if (line === undefined) throw new Error("Sentry anchor missing from the source");
  expect(planEdited("toolu", DEBUG, (skill) => `${skill}\n${line}\n`)).toThrow(
    `opencode port: plugins/toolu/skills/debug/SKILL.md: expected 1 match(es), found 2: ${SENTRY_ANCHOR}`,
  );
});
