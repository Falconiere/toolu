/**
 * The model-routing surface outside the SessionStart context (#263, ported
 * from model-routing.bats): the tier each pre-built agent pins in its
 * frontmatter, and the delivery references that carry the rubric.
 */
import { expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { PLUGIN } from "./lifecycle-cases.ts";

const AGENTS = join(PLUGIN, "agents");
const REFS = resolve(PLUGIN, "../delivery-flow/skills/delivery-flow/references");

function agent(name: string): string {
  return readFileSync(join(AGENTS, `${name}.md`), "utf8");
}

test("the tier ladder is pinned in frontmatter, cheapest to most capable", () => {
  const tiers = {
    "quick-task": "haiku",
    "deep-explore": "sonnet",
    "research-agent": "sonnet",
    implementer: "sonnet",
    architect: "opus",
  };
  for (const [name, tier] of Object.entries(tiers)) {
    expect(agent(name)).toMatch(new RegExp(`^model: ${tier}$`, "m"));
  }
});

test("every pre-built agent declares exactly one model", () => {
  const files = readdirSync(AGENTS).filter((file) => file.endsWith(".md"));
  expect(files.length).toBeGreaterThan(0);
  for (const file of files) {
    expect(readFileSync(join(AGENTS, file), "utf8").match(/^model: /gm)).toHaveLength(1);
  }
});

test("the cheap and mid tiers document the escalation path", () => {
  expect(agent("quick-task")).toContain("ESCALATE");
  expect(agent("implementer")).toContain("ESCALATE");
});

test("the top tier is read-only (no edit tools)", () => {
  expect(agent("architect")).not.toMatch(/^tools:.*(Edit|Write)/m);
});

test("the private delivery references remain self-contained", () => {
  expect(existsSync(join(REFS, "model-routing.md"))).toBe(true);
  for (const phase of ["plan", "execution"]) {
    expect(readFileSync(join(REFS, `${phase}.md`), "utf8")).toContain("model-routing.md");
  }
});
