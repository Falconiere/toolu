import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, realpathSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { applySurfaces } from "../apply.ts";
import { isPlainRecord } from "../merge.ts";
import { planSurfaces, type SurfacePlan } from "../plan.ts";

const GENERATED = realpathSync(join(import.meta.dir, "../../../generated"));
const tmpBase = process.env.TMPDIR ?? "/tmp";
const RESOURCES = `${GENERATED}/resources/*`;

function plan(selected: string[]): SurfacePlan {
  const result = planSurfaces(GENERATED, selected);
  if (!result.ok) throw new Error(result.reason);
  return result.plan;
}

/** An isolated project, HOME and XDG root: no skill exists anywhere the host looks. */
function scope(): { directory: string; worktree: string; env: Record<string, string> } {
  const base = realpathSync(mkdtempSync(join(tmpBase, "toolu-apply-")));
  const project = join(base, "project");
  mkdirSync(project, { recursive: true });
  return {
    directory: project,
    worktree: project,
    env: { HOME: join(base, "home"), XDG_CONFIG_HOME: join(base, "xdg") },
  };
}

/** `value` as a record; the test fails when the config holds anything else there. */
function rec(value: unknown): Record<string, unknown> {
  if (!isPlainRecord(value)) throw new Error(`not a record: ${JSON.stringify(value)}`);
  return value;
}

function put(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

test("an empty config gains the selected skills, agents, commands and the resources allow", () => {
  const config: Record<string, unknown> = {};
  const planned = plan(["pr-babysit", "toolu"]);
  const report = applySurfaces(config, planned, scope());
  expect(config.skills).toEqual({ paths: planned.skills.map((s) => s.dir) });
  expect(Object.keys(rec(config.agent))).toEqual(planned.agents.map((a) => a.id));
  expect(Object.keys(rec(config.command))).toEqual(planned.commands.map((c) => c.id));
  expect(config.permission).toEqual({ external_directory: { [RESOURCES]: "allow" } });
  expect(report).toMatchObject({ kept: [], merged: [], resourcesAllowed: true, notes: [] });
  expect(report.skills).toHaveLength(7);
});

test("unrelated entries, existing skill paths and urls, and other keys survive", () => {
  const mine = { description: "Mine", mode: "subagent", prompt: "mine" };
  const config: Record<string, unknown> = {
    agent: { mine },
    command: { mycmd: { template: "do it" } },
    skills: { paths: ["/opt/skills"], urls: ["https://example.test/skills"] },
    permission: { bash: "ask" },
    model: "probe/scripted",
  };
  applySurfaces(config, plan(["toolu"]), scope());
  expect(rec(config.agent).mine).toEqual(mine);
  expect(rec(config.command).mycmd).toEqual({ template: "do it" });
  const skills = rec(config.skills);
  expect(skills.urls).toEqual(["https://example.test/skills"]);
  expect(Array.isArray(skills.paths) && skills.paths[0]).toBe("/opt/skills");
  expect(config.permission).toEqual({ bash: "ask", external_directory: { [RESOURCES]: "allow" } });
  expect(config.model).toBe("probe/scripted");
});

test("user entries win key by key over toolu's, as with the host's built-ins", () => {
  const config: Record<string, unknown> = {
    agent: {
      "toolu-quick-task": { description: "My description" },
      "toolu-implementer": {
        prompt: "My prompt",
        description: "Mine",
        permission: { webfetch: "allow", read: "deny" },
      },
      "toolu-architect": { disable: true },
    },
    command: { "toolu-commit-1e9b92d5": { template: "my template" } },
  };
  const report = applySurfaces(config, plan(["toolu"]), scope());
  const agents = rec(config.agent);
  const quick = rec(agents["toolu-quick-task"]);
  expect(quick.description).toBe("My description");
  expect(String(quick.prompt).startsWith("## Instructions")).toBe(true);
  const implementer = rec(agents["toolu-implementer"]);
  expect(implementer.prompt).toBe("My prompt");
  expect(implementer.mode).toBe("subagent");
  const permission = rec(implementer.permission);
  expect(Object.keys(permission).at(-1)).toBe("webfetch");
  expect(permission.read).toBe("deny");
  expect(permission["*"]).toBe("deny");
  expect(rec(agents["toolu-architect"]).disable).toBe(true);
  const commit = rec(config.command)["toolu-commit-1e9b92d5"];
  expect(commit).toEqual({ description: "Commit all changes", template: "my template" });
  expect(report.merged).toEqual([
    { kind: "agent", id: "toolu-architect" },
    { kind: "agent", id: "toolu-implementer" },
    { kind: "agent", id: "toolu-quick-task" },
    { kind: "command", id: "toolu-commit-1e9b92d5" },
  ]);
});

test("a skill the host already discovers keeps the user's copy, from files or configured paths", () => {
  const s = scope();
  const userSkill = join(s.directory, ".opencode/skills/toolu-debug/SKILL.md");
  put(userSkill, "---\nname: toolu-debug\ndescription: mine\n---\n");
  const configured = join(s.directory, "../manual");
  put(
    join(configured, "toolu-orchestrator/SKILL.md"),
    "---\nname: toolu-orchestrator\ndescription: old\n---\n",
  );
  const config: Record<string, unknown> = { skills: { paths: [configured] } };
  const report = applySurfaces(config, plan(["toolu"]), s);
  expect(report.kept).toEqual([
    { id: "toolu-debug", location: userSkill },
    { id: "toolu-orchestrator", location: join(configured, "toolu-orchestrator/SKILL.md") },
  ]);
  const paths = rec(config.skills).paths;
  if (!Array.isArray(paths)) throw new Error("skills.paths is not an array");
  expect(paths.some((p) => String(p).endsWith("/skills/toolu-debug"))).toBe(false);
  expect(paths[0]).toBe(configured);
  expect(report.skills).toHaveLength(4);
});

test("any user rule that could match external_directory means toolu adds no allow", () => {
  for (const permission of [
    { external_directory: { "/elsewhere/*": "allow" } },
    { external_directory: "ask" },
    { "*": "ask" },
    { "external_*": "deny" },
  ]) {
    const config: Record<string, unknown> = { permission };
    const report = applySurfaces(config, plan(["toolu"]), scope());
    expect(config.permission).toEqual(permission);
    expect(report.resourcesAllowed).toBe(false);
    expect(report.notes).toEqual([
      "external_directory for toolu's shared procedures left to your permission rules",
    ]);
  }
});

test("malformed keys are left alone with a note and the rest still applies", () => {
  const config: Record<string, unknown> = { skills: "nope", command: ["x"] };
  const report = applySurfaces(config, plan(["toolu"]), scope());
  expect(config.skills).toBe("nope");
  expect(config.command).toEqual(["x"]);
  expect(Object.keys(rec(config.agent))).toHaveLength(5);
  expect(report.notes).toEqual([
    "skills config unreadable; no toolu skills added",
    "command config unreadable; no toolu commands added",
  ]);
  expect(config.permission).toBeUndefined();
});

test("a plugin with no surfaces changes nothing", () => {
  const config: Record<string, unknown> = {};
  applySurfaces(config, plan(["ts-quality"]), scope());
  expect(config).toEqual({});
});
