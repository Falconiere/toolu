/** One public delivery-flow skill owns every phase; its prose carries the delivery contract. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { Glob } from "bun";

const ROOT = join(import.meta.dir, "..", "..", "..", "..");
const PLUGIN = join(ROOT, "plugins", "delivery-flow");
const REFS = join(PLUGIN, "skills", "delivery-flow", "references");
const skill = () => readFileSync(join(PLUGIN, "skills", "delivery-flow", "SKILL.md"), "utf8");

test.concurrent("one public skill owns every required phase and private guidance", () => {
  const skills = [...new Glob("**/SKILL.md").scanSync(join(PLUGIN, "skills"))];
  expect(skills).toEqual(["delivery-flow/SKILL.md"]);
  const text = skill();
  for (const phase of ["spec", "spec-review", "plan", "plan-review", "execution", "test"]) {
    expect(existsSync(join(REFS, `${phase}.md`))).toBe(true);
    expect(existsSync(join(ROOT, "plugins", "toolu", "skills", phase, "SKILL.md"))).toBe(false);
    expect(text).toContain(`references/${phase}.md`);
  }
  expect(text).toContain("brainstorm → spec → spec review → plan → plan review → execution");
});

test.concurrent("brainstorm phase runs the brainstorm plugin instead of a private copy", () => {
  expect(existsSync(join(REFS, "brainstorm.md"))).toBe(false);
  expect(existsSync(join(REFS, "design-questions.md"))).toBe(false);
  const text = skill();
  expect(text).toContain("brainstorm:brainstorm");
  expect(text).toContain("installed `brainstorm`");
  for (const host of [".claude-plugin", ".codex-plugin"]) {
    const manifest = JSON.parse(readFileSync(join(PLUGIN, host, "plugin.json"), "utf8")) as {
      dependencies: { name: string }[];
    };
    expect(manifest.dependencies.map((d) => d.name)).toContain("brainstorm");
  }
});

test.concurrent("delivery stops at failed reviews and resumes from the failed phase", () => {
  const text = skill();
  for (const phrase of [
    "Status: Needs changes",
    "resume from that phase",
    'plan-ledger.sh" preflight',
    "real-data",
  ]) {
    expect(text).toContain(phrase);
  }
});

test.concurrent("invocation authorizes checked delivery and prerequisite failures block it", () => {
  const text = skill();
  for (const phrase of [
    "Invoking this skill authorizes",
    "gh api user",
    "non-default branch",
    "toolu-review:review",
    'verdict.sh" status',
    "overall: ready",
    "PR",
    "pr-babysit:babysit",
  ]) {
    expect(text).toContain(phrase);
  }
});
