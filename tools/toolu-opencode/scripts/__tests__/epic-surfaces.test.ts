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
// the skill, its references or the `epic` command, every skill it names
// resolves, and every action is a `toolu epic` verb (#435).

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

test("every skill the orchestrator names resolves, and it runs toolu epic verbs only", () => {
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

  expect(text.match(/\b[a-z-]+\.ts\b/g) ?? []).toEqual([]);
  expect(text).not.toContain("scripts/");
  expect(existsSync(join(staged, "epic-orchestrator", "scripts"))).toBe(false);
  const verbs = new Set([...text.matchAll(/toolu (?:--json )?epic ([a-z]+)/g)].map(([, v = ""]) => v));
  for (const verb of ["graph", "route", "launch", "start", "wait", "gate", "release", "finish"]) {
    expect(verbs).toContain(verb);
  }
});

test("the orchestrator waits for judgments in the foreground", () => {
  const skill = read(`${EPIC}/SKILL.md`);
  expect(skill).toContain(
    "toolu --json epic wait --max-seconds 480    # Bash tool with a 600000 ms timeout",
  );
  expect(skill.replaceAll(/\s+/g, " ")).toContain(
    "run it again in the same turn, and keep that loop going while any issue is active.",
  );
  expectHostValidSkill(EPIC);
});

const SKILL = "skills/epic-orchestrator/SKILL.md";
const DEPENDENCIES = [
  "- Your skill list includes `delivery-flow:delivery-flow` and its `toolu`,",
  "  `toolu-review`, `pr-babysit`, and `brainstorm` dependencies (or the OpenCode-generated equivalents). Workers run as",
  "  herdr agents and need those plugins installed in every host in `--hosts`",
  "  (`npx @toolu/plugins install delivery-flow --host codex`, and so on).",
].join("\n");
const ANCHOR = DEPENDENCIES.split("\n")[0] ?? "";

test("generation fails naming the source when an epic-orchestrator anchor is gone", () => {
  expect(
    planEdited("epic-orchestrator", SKILL, (text) => text.replace(ANCHOR, "- Skills:")),
  ).toThrow(
    `opencode port: plugins/epic-orchestrator/${SKILL}: expected 1 match(es), found 0: ${ANCHOR}`,
  );
});

test("generation fails naming the source when an epic-orchestrator anchor repeats", () => {
  expect(readFileSync(join(ROOT, "plugins/epic-orchestrator", SKILL), "utf8")).toContain(
    DEPENDENCIES,
  );
  expect(planEdited("epic-orchestrator", SKILL, (text) => `${text}\n${DEPENDENCIES}\n`)).toThrow(
    `opencode port: plugins/epic-orchestrator/${SKILL}: expected 1 match(es), found 2: ${ANCHOR}`,
  );
});
