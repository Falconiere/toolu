/**
 * epic-orchestrator on OpenCode (#356): selecting it alone brings the delivery
 * chain's skills, the generated skill's root lines find its scripts in the
 * agent's bash, and a worker's native `task` delegation reaches agent-tier —
 * recorded, and refused when its model contradicts the running plan step.
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
/** The generated skill's ROOT and S lines, exactly as the model runs them. */
const ROOT_LINES = /```bash\n(# OpenCode's bash[^`]*?S="\$\{ROOT\}\/scripts")\n```/.exec(
  SKILL,
)?.[1];

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

test.concurrent("the skill's root lines reach its scripts, and stop when the plugin is off", async () => {
  expect(ROOT_LINES).toBeDefined();
  using sb = createSandbox({ git: true });
  epicProject(sb, ["epic-orchestrator"]);
  const status = join(sb.root, "state/status/k.json");
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const res = await bash(hooks, sb, `${ROOT_LINES}\nbun "$S/report.ts" "${status}" execution`);
    expect(res.stderr).toBe("");
    expect(res.exitCode).toBe(0);
  });
  expect(JSON.parse(readFileSync(status, "utf8"))).toMatchObject({ phase: "execution" });

  using off = createSandbox({ git: true });
  epicProject(off, ["toolu"]);
  await withHooks(binding(off, [], ""), async (hooks) => {
    const res = await bash(hooks, off, `${ROOT_LINES}\necho reached`);
    expect(res.exitCode).not.toBe(0);
    expect(res.stderr).toContain("epic-orchestrator is not enabled in this OpenCode session");
    expect(res.stdout).not.toContain("reached");
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
