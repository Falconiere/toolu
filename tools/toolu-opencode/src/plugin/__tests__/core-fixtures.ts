/**
 * Shared by the hermetic and live core-workflow suites (#358): a git project
 * selecting toolu and toolu-review with a local bare remote, and the exact
 * commands the generated review and debug skills tell the model to run.
 */
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { gitProject } from "./workflow-fixtures.ts";

export const BRANCH = "feat/review";
export const GENERATED = join(REPO_ROOT, "tools/toolu-opencode/generated");
export const SELECTION = { version: 1, enabled: ["toolu", "toolu-review"] };
export const FAILING_NAME = "adds two numbers";
export const FAILING_TEST = `import { expect, test } from "bun:test";\n\ntest("${FAILING_NAME}", () => {\n  expect(1 + 1).toBe(3);\n});\n`;
export const PASSING_TEST = FAILING_TEST.replace("toBe(3)", "toBe(2)");

const generatedSkill = (id: string): string =>
  readFileSync(join(GENERATED, "skills", id, "SKILL.md"), "utf8");

/** The generated review skill's OpenCode write-state command, recording `findings` open findings. */
export function writeStateCommand(findings: number): string {
  const skill = generatedSkill("toolu-review-review");
  const block = skill.split("   # OpenCode\n")[1]?.split("   ```")[0];
  if (block === undefined) throw new Error("no OpenCode write-state block in toolu-review-review");
  return block
    .split("\n")
    .map((line) => line.slice(3))
    .join("\n")
    .replace("--findings-count 0", `--findings-count ${findings}`)
    .trim();
}

/** The generated debug skill's pipe from `bun test` into its test-failure collector. */
export function debugTestfailCommand(): string {
  const hit =
    /`(bun test 2>&1 \| bun "\$TOOLU_PLUGIN_ROOT_TOOLU\/scripts\/debug-testfail\.ts")`/.exec(
      generatedSkill("toolu-debug"),
    );
  if (hit?.[1] === undefined) throw new Error("no bun test pipe in toolu-debug");
  return hit[1];
}

/**
 * `sb` (created with `git: true`) as a project selecting toolu and toolu-review
 * with `gates`, `main` pushed to a bare remote, and `BRANCH` one commit ahead.
 * Returns the remote's path.
 */
export function reviewProject(sb: Sandbox, gates: object = {}): string {
  return gitProject(sb, {
    branch: BRANCH,
    selection: SELECTION,
    gates,
    files: { "math.test.ts": PASSING_TEST },
  });
}
