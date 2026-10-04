import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { stagePlugins } from "../bundle-plugins.ts";
import {
  GENERATED,
  ROOT,
  expectHostValidSkill,
  generatedIds,
  planEdited,
  read,
  skillNames,
} from "./surface-audit.ts";

// The epic-orchestrator skill an OpenCode orchestrator loads (#356), audited on
// the committed generated tree: no Claude-only step (background Bash, turn
// re-invocation, plugin-root fallbacks, other hosts' skill syntax) survives in
// the skill, its references or the `epic` command, and every skill and script
// it names resolves.

const EPIC = "skills/epic-orchestrator-epic-orchestrator";
const SURFACES = [
  `${EPIC}/SKILL.md`,
  ...readdirSync(join(GENERATED, EPIC, "references")).map((name) => `${EPIC}/references/${name}`),
  "commands/epic-orchestrator-epic.md",
];

const BANNED = [
  "run_in_background",
  "end your turn",
  "re-invoked",
  "CLAUDE_PLUGIN_ROOT",
  "${PLUGIN_ROOT",
  "npx @toolu/plugins",
  "delivery-flow:delivery-flow",
  "pr-babysit:babysit",
  "toolu:",
];
const DOUBLE_HYPHEN_ID = /`[a-z0-9]+(?:-[a-z0-9]+)*--[a-z0-9-]+`/g;

test("the skill ships both references and the epic command", () => {
  expect(SURFACES.toSorted()).toEqual([
    "commands/epic-orchestrator-epic.md",
    `${EPIC}/SKILL.md`,
    `${EPIC}/references/recovery.md`,
    `${EPIC}/references/worker-brief.md`,
  ]);
});

test.each(SURFACES)("%s names no Claude-only step or other host's skill syntax", (rel) => {
  const text = read(rel);
  expect(BANNED.filter((token) => text.includes(token))).toEqual([]);
  expect(text.match(DOUBLE_HYPHEN_ID) ?? []).toEqual([]);
});

test("every skill and script the orchestrator names resolves", () => {
  using sb = createSandbox();
  const staged = join(sb.root, "staged");
  stagePlugins(join(ROOT, "plugins"), staged);
  const { skills } = generatedIds();
  const text = SURFACES.map(read).join("\n");

  const named = new Set(skillNames(text));
  for (const id of [
    "delivery-flow-delivery-flow",
    "brainstorm-brainstorm",
    "toolu-review-review",
    "pr-babysit-babysit-73c340c6",
  ]) {
    expect(named).toContain(id);
  }
  expect([...named].filter((id) => !skills.has(id))).toEqual([]);

  const scripts = new Set([...text.matchAll(/"\$S\/([a-z-]+\.ts)"/g)].map(([, s = ""]) => s));
  expect(scripts).toContain("epic-watch.ts");
  expect(scripts).toContain("launch-issue.ts");
  for (const script of scripts) {
    expect(existsSync(join(staged, "epic-orchestrator", "scripts", script))).toBe(true);
  }
});

test("the orchestrator finds its root through shell.env and watches in the foreground", () => {
  const skill = read(`${EPIC}/SKILL.md`);
  expect(skill).toContain(
    'ROOT="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?epic-orchestrator is not enabled in this OpenCode session}"\nS="${ROOT}/scripts"',
  );
  expect(skill).toContain(
    'bun "$S/epic-watch.ts" --state-dir <state_dir> --max-wait 480   # bash tool with timeout: 600000',
  );
  expect(skill).toContain(
    "run it again in the same turn, and keep that loop going while any\nissue is active.",
  );
  expectHostValidSkill(EPIC);
});

const SKILL = "skills/epic-orchestrator/SKILL.md";
const WATCH_ANCHOR = "## 3. Watch (background)";

test("generation fails naming the source when an epic-orchestrator anchor is gone", () => {
  expect(
    planEdited("epic-orchestrator", SKILL, (text) => text.replace(WATCH_ANCHOR, "## 3. Watch")),
  ).toThrow(
    `opencode port: plugins/epic-orchestrator/${SKILL}: expected 1 match(es), found 0: ${WATCH_ANCHOR}`,
  );
});

test("generation fails naming the source when an epic-orchestrator anchor repeats", () => {
  expect(readFileSync(join(ROOT, "plugins/epic-orchestrator", SKILL), "utf8")).toContain(
    WATCH_ANCHOR,
  );
  expect(planEdited("epic-orchestrator", SKILL, (text) => `${text}\n${WATCH_ANCHOR}\n`)).toThrow(
    `opencode port: plugins/epic-orchestrator/${SKILL}: expected 1 match(es), found 2: ${WATCH_ANCHOR}`,
  );
});
