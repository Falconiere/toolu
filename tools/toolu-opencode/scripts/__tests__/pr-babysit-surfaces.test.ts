import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { expect, test } from "bun:test";
import { applyPort } from "../lib/opencode-port.ts";
import { PR_BABYSIT_PORTS } from "../lib/opencode-port-pr-babysit.ts";

// pr-babysit's OpenCode surfaces (#357), audited on the committed generated
// tree: the OpenCode controller only, every link resolves, and the shared
// authorization contract is kept word for word.

const ROOT = resolve(import.meta.dir, "../../../..");
const GENERATED = join(ROOT, "tools/toolu-opencode/generated");
const SKILL = "skills/pr-babysit-babysit-73c340c6/SKILL.md";
const COMMAND = "commands/pr-babysit-babysit-ff6e5a3d.md";
const WORKFLOW = "resources/pr-babysit/workflows/babysit.md";
const SOURCE_WORKFLOW = "plugins/pr-babysit/workflows/babysit.md";

const read = (rel: string): string => readFileSync(join(GENERATED, rel), "utf8");

/** The surfaces plus every pr-babysit Markdown file reachable from them; shared docs of other plugins are not pr-babysit's. */
function closure(): string[] {
  const seen = new Set<string>();
  const queue = [SKILL, COMMAND];
  for (let rel = queue.shift(); rel !== undefined; rel = queue.shift()) {
    if (seen.has(rel)) continue;
    seen.add(rel);
    for (const [, target = ""] of read(rel).matchAll(/\]\(([^)#\s]+\.md)(?:#[^)]*)?\)/g)) {
      const next = join(dirname(rel), target);
      if (/^(skills\/pr-babysit-|resources\/pr-babysit\/)/.test(next)) queue.push(next);
    }
  }
  return [...seen].toSorted();
}

const CLOSURE = closure();

const BANNED = [
  "get_goal",
  "create_goal",
  "update_goal",
  "CronCreate",
  "CronList",
  "CronDelete",
  "EnterWorktree",
  "ExitWorktree",
  "spawn_agent",
  "CODEX_HOME",
  ".codex/tmp",
  "/tmp/pr-babysit-${SLOT}",
  "/tmp/pr-babysit-<slot>",
  "--host claude",
  "--host codex",
  "CLAUDE_PLUGIN_ROOT",
  "### Claude Code scheduling",
  "### Codex",
  'bun "$PLUGIN_ROOT',
  "`--host` is this controller",
  "Claude cron interval",
];

test("the closure reaches the workflow and helper contract; the fixer brief ships beside them", () => {
  expect(CLOSURE).toContain(WORKFLOW);
  expect(CLOSURE).toContain("skills/pr-babysit-babysit-73c340c6/references/helper.md");
  expect(
    existsSync(join(GENERATED, "skills/pr-babysit-babysit-73c340c6/references/fixer-brief.md")),
  ).toBe(true);
});

test.each(CLOSURE)("%s carries no Claude Code or Codex controller", (rel) => {
  const text = read(rel);
  expect(BANNED.filter((token) => text.includes(token))).toEqual([]);
});

test.each(CLOSURE)("%s links only to files that exist", (rel) => {
  const missing = [...read(rel).matchAll(/\]\(([^)#\s:]+)(?:#[^)]*)?\)/g)]
    .map(([, target = ""]) => target)
    .filter((target) => !existsSync(join(GENERATED, dirname(rel), target)));
  expect(missing).toEqual([]);
});

test("the skill and workflow name the OpenCode controller, routing, state and helper root", () => {
  const skill = read(SKILL);
  for (const needle of [
    "--host opencode",
    "$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json",
    '"$TOOLU_PLUGIN_ROOT_PR_BABYSIT"',
    '"$TOOLU_BUN" --no-env-file',
    "sleep <backoff.waitSeconds>",
    "wait --timeout-seconds 45",
  ])
    expect(skill).toContain(needle);
  expect(skill).toContain('description: "Use when the user explicitly asks, or an authorized');
  const workflow = read(WORKFLOW);
  for (const needle of [
    "### OpenCode start or resume",
    "### OpenCode cancel",
    "`PLUGIN_ROOT` = `$TOOLU_PLUGIN_ROOT_PR_BABYSIT`",
    "--host opencode \\",
    '`task` with `subagent_type: "toolu-implementer"`',
    "$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.inline",
    "<worktree>/.opencode/tmp/push-review/",
    "pr-babysit-fixer",
  ])
    expect(workflow).toContain(needle);
});

test("the shared authorization and execution handoff section is kept verbatim", () => {
  const section = (text: string): string => {
    const start = text.indexOf("## Authorization and execution handoff");
    return text.slice(start, text.indexOf("\n## ", start + 1));
  };
  const source = readFileSync(join(ROOT, SOURCE_WORKFLOW), "utf8");
  expect(section(source).length).toBeGreaterThan(100);
  expect(section(read(WORKFLOW))).toBe(section(source));
});

test("a moved anchor fails generation and names the source", () => {
  const source = readFileSync(join(ROOT, SOURCE_WORKFLOW), "utf8");
  expect(() =>
    applyPort(SOURCE_WORKFLOW, source.replace("### Claude Code scheduling", "### Claude")),
  ).toThrow(
    `opencode port: ${SOURCE_WORKFLOW}: expected 1 match(es), found 0: ### Claude Code scheduling`,
  );
  expect(() =>
    applyPort(SOURCE_WORKFLOW, source.replace("### OpenCode start or resume\n", "")),
  ).toThrow("found 0: ### OpenCode start or resume");
  expect(Object.keys(PR_BABYSIT_PORTS).toSorted()).toEqual([
    "plugins/pr-babysit/skills/babysit/SKILL.md",
    "plugins/pr-babysit/skills/babysit/references/helper.md",
    SOURCE_WORKFLOW,
  ]);
});
