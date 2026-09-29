/** The skill, command and worker brief name only scripts that exist, all run through bun. */

import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..");
const SKILL_DIR = join(ROOT, "skills", "epic-orchestrator");
const read = (...parts: string[]) => readFileSync(join(...parts), "utf8");
const skill = () => read(SKILL_DIR, "SKILL.md");

test.concurrent("skill invokes bun CLIs via host plugin root", () => {
  const text = skill();
  for (const line of [
    'ROOT="${CLAUDE_PLUGIN_ROOT}"',
    'S="${ROOT}/scripts"',
    'bun "$S/epic-graph.ts"',
    'bun "$S/launch-issue.ts"',
    'bun "$S/epic-watch.ts"',
    'bun "$S/merge-gate.ts"',
    'bun "$S/route.ts"',
    'bun "$S/epic-close.ts"',
    'bun "$S/finish-issue.ts"',
  ]) {
    expect(text).toContain(line);
  }
  expect(text).not.toContain("python3");
  expect(text).not.toMatch(/\.py\b/);
  expect(read(SKILL_DIR, "references", "recovery.md")).not.toMatch(/\.py\b/);
  expect(text).not.toContain("~/.claude/skills/epic-orchestrator");
});

test.concurrent("command points at plugin skill and bun scripts", () => {
  const cmd = read(ROOT, "commands", "epic.md");
  expect(cmd).toContain("CLAUDE_PLUGIN_ROOT");
  expect(cmd).toContain("bun");
  expect(cmd).not.toMatch(/\.sh\b/);
});

test.concurrent("preflight requires bun", () => {
  expect(skill()).toContain("command -v bun");
});

test.concurrent("every script the skill names exists and none is a shell script", () => {
  const named = [...skill().matchAll(/"\$S\/([a-z_-]+\.[a-z]+)"/g)].map((m) => m[1] ?? "");
  expect(named.length).toBeGreaterThan(0);
  for (const script of new Set(named)) {
    expect(script).toEndWith(".ts");
    expect(existsSync(join(ROOT, "scripts", script))).toBe(true);
  }
});

test.concurrent("brief placeholders are all filled by the launcher", () => {
  const launcher = read(ROOT, "scripts", "launch-issue.ts");
  const brief = read(SKILL_DIR, "references", "worker-brief.md");
  const placeholders = new Set([...brief.matchAll(/\{\{([A-Z_]+)\}\}/g)].map((m) => m[1] ?? ""));
  expect(placeholders.size).toBeGreaterThan(0);
  for (const ph of placeholders) {
    expect(launcher).toContain(`    ${ph}:`);
  }
});
