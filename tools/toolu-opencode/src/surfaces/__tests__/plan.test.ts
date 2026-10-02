import { expect, test } from "bun:test";
import { cpSync, mkdtempSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { planSurfaces } from "../plan.ts";

const GENERATED = realpathSync(join(import.meta.dir, "../../../generated"));
const tmpBase = process.env.TMPDIR ?? "/tmp";

function plan(selected: string[], dir = GENERATED) {
  const result = planSurfaces(dir, selected);
  if (!result.ok) throw new Error(result.reason);
  return result.plan;
}

test("pr-babysit with its toolu dependency plans 7 skills, 5 agents and 3 commands in catalog order", () => {
  const planned = plan(["toolu", "pr-babysit"]);
  expect(planned.plugins).toEqual(["pr-babysit", "toolu"]);
  expect(planned.skills.map((s) => s.id)).toEqual([
    "pr-babysit-babysit-73c340c6",
    "toolu-commit-e11d9d00",
    "toolu-debug",
    "toolu-deep-research",
    "toolu-orchestrator",
    "toolu-review-and-commit-1a591621",
    "toolu-setup",
  ]);
  expect(planned.skills[2]?.dir).toBe(join(GENERATED, "skills/toolu-debug"));
  expect(planned.agents.map((a) => a.id)).toHaveLength(5);
  expect(planned.commands.map((c) => c.id)).toEqual([
    "pr-babysit-babysit-ff6e5a3d",
    "toolu-commit-1e9b92d5",
    "toolu-review-and-commit-db159d0c",
  ]);
  expect(planned.notes).toEqual([]);
});

test("entries carry the host loader's fields: frontmatter plus the trimmed body", () => {
  const planned = plan(["toolu"]);
  const quick = planned.agents.find((a) => a.id === "toolu-quick-task")?.entry;
  expect(quick?.mode).toBe("subagent");
  expect(quick?.permission).toEqual({
    "*": "deny",
    bash: "allow",
    glob: "allow",
    grep: "allow",
    read: "allow",
  });
  expect(String(quick?.prompt).startsWith("## Instructions")).toBe(true);
  expect(Object.keys(quick ?? {}).toSorted()).toEqual([
    "description",
    "mode",
    "permission",
    "prompt",
  ]);
  const commit = planned.commands.find((c) => c.id === "toolu-commit-1e9b92d5")?.entry;
  expect(commit).toEqual({
    description: "Commit all changes",
    template:
      "Load the `toolu-commit-e11d9d00` skill with the native skill tool and follow its instructions. Pass $ARGUMENTS as task context.",
  });
});

test("a plugin without surfaces plans nothing; an unknown name is a note", () => {
  expect(plan(["ts-quality"]).skills).toEqual([]);
  expect(plan(["jev", "no-such-plugin"]).notes).toEqual([
    'no generated surface for plugin "no-such-plugin"',
  ]);
});

test("non-canonical or unexpected agent frontmatter fails the plan", () => {
  const variants: Array<[string, (text: string) => string, string]> = [
    [
      "yaml",
      (text) => text.replace('mode: "subagent"', "mode: subagent"),
      'value of "mode" is not JSON',
    ],
    ["key", (text) => text.replace('mode: "subagent"', 'mode: "subagent"\nnope: true'), "nope"],
  ];
  for (const [label, mutate, expected] of variants) {
    const dir = join(realpathSync(mkdtempSync(join(tmpBase, "toolu-plan-"))), "generated");
    cpSync(GENERATED, dir, { recursive: true });
    const agent = join(dir, "agents/toolu-quick-task.md");
    writeFileSync(agent, mutate(readFileSync(agent, "utf8")));
    const result = planSurfaces(dir, ["toolu"]);
    expect({ label, ok: result.ok }).toEqual({ label, ok: false });
    if (!result.ok) expect(result.reason).toContain(expected);
  }
});
