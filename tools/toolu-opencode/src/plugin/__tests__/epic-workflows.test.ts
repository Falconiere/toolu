/**
 * epic-orchestrator on OpenCode (#356): selecting it alone brings the delivery
 * chain's skills, the generated skill runs `toolu epic` verbs (#435) while the
 * plugin root reaches its references only when enabled, and a worker's native
 * `task` delegation reaches agent-tier — recorded, and refused when its model
 * contradicts the running plan step.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "bun:test";
import type { Config, Hooks } from "@opencode-ai/plugin";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { isPlainRecord } from "../../surfaces/merge.ts";
import { GENERATED } from "./core-fixtures.ts";
import { binding, hook } from "./jev-fixtures.ts";
import { bash, gitProject, withHooks } from "./workflow-fixtures.ts";

const BRANCH = "feat/epic";
const TELEMETRY = ".opencode/tmp/telemetry/feat_epic.jsonl";
const LEDGER = ".opencode/tmp/plan-ledger/feat_epic.json";
const CALL = { sessionID: "ses_epic", callID: "call_epic" };
const SKILL = readFileSync(
  join(GENERATED, "skills/epic-orchestrator-epic-orchestrator/SKILL.md"),
  "utf8",
);
/** The references the plugin root exposes to the agent's bash. */
const BRIEF = '"$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR/skills/epic-orchestrator/references/worker-brief.md"';

function epicProject(sb: Sandbox, enabled: string[], gates: object = {}): string {
  return gitProject(sb, {
    branch: BRANCH,
    selection: { version: 1, enabled },
    gates,
    files: { "README.md": "# epic fixture\n" },
  });
}

async function taskRefusal(hooks: Hooks, args: Record<string, string>): Promise<string> {
  try {
    await hook(hooks, "tool.execute.before")({ tool: "task", ...CALL }, { args });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "allowed";
}

test.concurrent("selecting epic-orchestrator alone registers it and the delivery chain", async () => {
  using sb = createSandbox({ git: true });
  epicProject(sb, ["epic-orchestrator"]);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const config: Config = {};
    await hook(hooks, "config")(config);
    const skills: unknown = Reflect.get(config, "skills");
    const paths = isPlainRecord(skills) && Array.isArray(skills.paths) ? skills.paths : [];
    for (const id of [
      "epic-orchestrator-epic-orchestrator",
      "delivery-flow-delivery-flow",
      "brainstorm-brainstorm",
      "toolu-review-review",
      "pr-babysit-babysit-73c340c6",
    ]) {
      expect(paths).toContain(`${GENERATED}/skills/${id}`);
    }
  });
});

test.concurrent("the skill runs toolu epic verbs, and its root reaches the references only when enabled", async () => {
  expect(SKILL).toContain('toolu epic start "$STATE_DIR"');
  expect(SKILL).not.toContain("$S/");
  using sb = createSandbox({ git: true });
  epicProject(sb, ["epic-orchestrator"]);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const res = await bash(hooks, sb, `test -f ${BRIEF} && echo reached`);
    expect(res.stderr).toBe("");
    expect(res.stdout).toContain("reached");
  });

  using off = createSandbox({ git: true });
  epicProject(off, ["toolu"]);
  await withHooks(binding(off, [], ""), async (hooks) => {
    const res = await bash(hooks, off, 'echo "root=${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:-unset}"');
    expect(res.stdout).toContain("root=unset");
  });
});

test.concurrent("a native task is recorded, and refused when its model contradicts the plan step", async () => {
  using sb = createSandbox({ git: true });
  epicProject(sb, ["epic-orchestrator"], { agentTier: { mode: "block" } });
  sb.write(LEDGER, {
    next: "inspect",
    steps: [{ id: "inspect", status: "running", model: "haiku" }],
  });
  const task = {
    description: "inspect",
    prompt: "list the scripts",
    subagent_type: "toolu-quick-task",
  };
  await withHooks(binding(sb, [], ""), async (hooks) => {
    expect(await taskRefusal(hooks, { ...task, model: "opus" })).toContain(
      'plan step "inspect" expects model tier "haiku" but this delegation used "opus"',
    );
    expect(await taskRefusal(hooks, task)).toBe("allowed");
  });
  const lines = sb
    .read(TELEMETRY)
    .trim()
    .split("\n")
    .map((line): unknown => JSON.parse(line));
  expect(lines).toHaveLength(2);
  expect(lines[0]).toMatchObject({ event: "delegation", model: "opus", step_id: "inspect" });
  expect(lines[1]).toMatchObject({
    event: "delegation",
    subagent_type: "toolu-quick-task",
    model: null,
    step_id: "inspect",
    step_model: "haiku",
  });
});
