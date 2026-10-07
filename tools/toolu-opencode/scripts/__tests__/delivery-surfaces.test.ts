import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "bun:test";
import {
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

// The brainstorm and delivery-flow skills OpenCode loads (#355), audited on the
// committed generated tree: every reference they link is installed, no Claude
// Code or Codex interface or model argument leaks in, and every skill, agent
// and toolu path they hand the model resolves.

const BRAINSTORM = "skills/brainstorm-brainstorm";
const DELIVERY = "skills/delivery-flow-delivery-flow";
const HOST_MAPPING = `${DELIVERY}/references/host-mapping.md`;
const CLOSURE = linkedClosure([`${BRAINSTORM}/SKILL.md`, `${DELIVERY}/SKILL.md`]);

const BANNED = [
  "AskUserQuestion",
  "request_user_input",
  "spawn_agent",
  "EnterWorktree",
  "CODEX_HOME",
  "claude plugin",
  "codex plugin",
  "installPath",
  "toolu:",
  "pr-babysit:babysit",
  "toolu-review:review",
  "brainstorm:brainstorm",
  "Pass `model:`",
  "explicit `model:`",
  "Claude Code or Codex",
  "PostToolUse",
  "systematic-debugging",
];

const TIERS = [
  "`toolu-quick-task` (`haiku`)",
  "`toolu-implementer` (`sonnet`)",
  "`toolu-architect` (`opus`, `fable`)",
  "`general` (`inherit`)",
];

test("both skills link every reference shipped beside them", () => {
  const shipped = (dir: string): string[] =>
    readdirSync(join(ROOT, "tools/toolu-opencode/generated", dir, "references")).map(
      (name) => `${dir}/references/${name}`,
    );
  expect(shipped(DELIVERY)).toHaveLength(10);
  for (const rel of [...shipped(BRAINSTORM), ...shipped(DELIVERY)]) {
    expect(CLOSURE).toContain(rel);
  }
});

test.each(CLOSURE.filter((rel) => rel !== HOST_MAPPING))(
  "%s names no Claude Code or Codex interface",
  (rel) => {
    const text = read(rel);
    expect(BANNED.filter((token) => text.includes(token))).toEqual([]);
  },
);

test("every skill and agent the skills name resolves, and toolu runs as a binary", () => {
  const { skills, agents } = generatedIds();
  const text = CLOSURE.map(read).join("\n");

  const named = new Set(skillNames(text));
  for (const id of [
    "brainstorm-brainstorm",
    "delivery-flow-delivery-flow",
    "toolu-review-review",
    "jev-jev",
    "toolu-debug",
  ]) {
    expect(named).toContain(id);
  }
  expect([...named].some((id) => id.startsWith("pr-babysit-babysit"))).toBe(true);
  expect([...named].filter((id) => !skills.has(id))).toEqual([]);

  const quoted = [...text.matchAll(/[`"](toolu-[a-z0-9-]+)[`"]/g)].map(([, id = ""]) => id);
  expect(new Set(quoted)).toContain("toolu-deep-explore");
  expect(quoted.filter((id) => !agents.has(id) && !skills.has(id))).toEqual([]);

  // The ledger and verdict are `toolu ledger` verbs (#421), never a bundle path.
  expect(text).not.toContain("$TOOLU_PLUGIN_ROOT");
  for (const command of ["toolu ledger preflight", "toolu ledger verdict status"]) {
    expect(text).toContain(command);
  }
});

test("both skills carry frontmatter the host accepts", () => {
  expectHostValidSkill(BRAINSTORM);
  expectHostValidSkill(DELIVERY);
  expect(read(`${DELIVERY}/SKILL.md`)).toContain("babysit on OpenCode.");
});

test("delivery-flow runs toolu ledger and stops when bash cannot run toolu", () => {
  const skill = read(`${DELIVERY}/SKILL.md`);
  expect(skill).toContain("`toolu ledger …` and `toolu ledger verdict …`");
  expect(skill).toContain(
    "When bash cannot run `toolu`, toolu is not ready in this\nsession: stop and name that prerequisite.",
  );
});

test.each(["references/ledger.md", "references/execution.md"])(
  "%s gives each step tier its OpenCode agent and no model argument",
  (rel) => {
    const text = read(`${DELIVERY}/${rel}`).replaceAll("\n", " ").replaceAll(/ +/g, " ");
    for (const tier of TIERS) expect(text).toContain(tier);
    expect(text).toContain("`subagent_type`");
    expect(text).toContain("takes no model argument");
  },
);

test("delivery-flow's model routing is the ported toolu copy", () => {
  expect(read(`${DELIVERY}/references/model-routing.md`)).toBe(
    read("skills/toolu-orchestrator/references/model-routing.md"),
  );
  expect(read(`${DELIVERY}/references/semantic-judgments.md`)).toBe(
    read("resources/toolu/workflows/semantic-judgments.md"),
  );
});

test("delivery-flow's host mapping has toolu's OpenCode column and keeps the other cells", () => {
  const rows = hostMappingRows(HOST_MAPPING);
  expect(rows.length).toBe(HOST_CELLS.length);
  rows.forEach((row, index) => expect(row.startsWith(`${HOST_CELLS[index]} `)).toBe(true));
  expect(rows).toEqual(hostMappingRows("resources/toolu/workflows/host-mapping.md"));
});

test("brainstorm asks through OpenCode's question tool", () => {
  expect(read(`${BRAINSTORM}/SKILL.md`)).toContain(
    "OpenCode's `question` tool when it is listed; otherwise ask one concise plain\nquestion.",
  );
});

const LEDGER = "skills/delivery-flow/references/ledger.md";
const LEDGER_ANCHOR = "- `model`: `haiku`, `sonnet`, `opus`, `fable`, or `inherit`.";

test("generation fails naming the source when a delivery-flow anchor is gone", () => {
  expect(
    planEdited("delivery-flow", LEDGER, (text) =>
      text.replace(LEDGER_ANCHOR, "- `model`: a tier."),
    ),
  ).toThrow(
    `opencode port: plugins/delivery-flow/${LEDGER}: expected 1 match(es), found 0: ${LEDGER_ANCHOR}`,
  );
});

test("generation fails naming the source when a delivery-flow anchor repeats", () => {
  expect(readFileSync(join(ROOT, "plugins/delivery-flow", LEDGER), "utf8")).toContain(
    LEDGER_ANCHOR,
  );
  expect(planEdited("delivery-flow", LEDGER, (text) => `${text}\n${LEDGER_ANCHOR}\n`)).toThrow(
    `opencode port: plugins/delivery-flow/${LEDGER}: expected 1 match(es), found 2: ${LEDGER_ANCHOR}`,
  );
});
