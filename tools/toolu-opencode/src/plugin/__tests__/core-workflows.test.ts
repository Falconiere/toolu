/**
 * toolu's core workflows on OpenCode (#358): the review skill's write-state
 * command against the real push-review gate and a bare remote, the commit gate
 * after a real failing test run, and the debug skill's helper in the agent's
 * bash for the clone and the npm layout, all through `createTooluHooks`.
 */
import { expect, test } from "bun:test";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import type { Config } from "@opencode-ai/plugin";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { REPO_ROOT } from "../../bootstrap/__tests__/fixtures.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import { stagePlugins } from "../../../scripts/bundle-plugins.ts";
import {
  BRANCH,
  FAILING_NAME,
  FAILING_TEST,
  GENERATED,
  PASSING_TEST,
  debugTestfailCommand,
  reviewProject,
  writeStateCommand,
} from "./core-fixtures.ts";
import { bashEnv, binding, hook, inShell } from "./jev-fixtures.ts";
import { bash, refusal, remoteHead, withHooks } from "./workflow-fixtures.ts";

test.concurrent("toolu and toolu-review register 7 skills, 5 agents and 4 commands", async () => {
  using sb = createSandbox({ git: true });
  reviewProject(sb);
  await withHooks(binding(sb, [], ""), async (hooks) => {
    const config: Config = {};
    await hook(hooks, "config")(config);
    const skills: unknown = Reflect.get(config, "skills");
    const paths = isPlainRecord(skills) && Array.isArray(skills.paths) ? skills.paths : [];
    expect(new Set(paths)).toEqual(
      new Set(
        readdirSync(join(GENERATED, "skills"))
          .filter((id) => id.startsWith("toolu-"))
          .map((id) => join(GENERATED, "skills", id)),
      ),
    );
    expect(paths).toHaveLength(7);
    expect(Object.keys(config.agent ?? {}).toSorted()).toEqual([
      "toolu-architect",
      "toolu-deep-explore",
      "toolu-implementer",
      "toolu-quick-task",
      "toolu-research-agent",
    ]);
    expect(Object.keys(config.command ?? {})).toHaveLength(4);
  });
});

test.concurrent("the review skill's write-state command is what lets a blocked push through", async () => {
  using sb = createSandbox({ git: true });
  const remote = reviewProject(sb, { pushReview: { mode: "block" } });
  const push = `git push origin ${BRANCH}`;
  await withHooks(binding(sb, [], ""), async (hooks) => {
    expect(await refusal(hooks, push)).toContain("Code review required before push");
    expect(remoteHead(sb, remote, BRANCH)).toBe("");

    expect((await bash(hooks, sb, writeStateCommand(1))).exitCode).toBe(0);
    expect(await refusal(hooks, push)).toContain("Code review has open findings (1)");

    const written = await bash(hooks, sb, writeStateCommand(0));
    expect(written).toMatchObject({ exitCode: 0 });
    const state: unknown = JSON.parse(sb.read(".opencode/tmp/push-review/feat_review.json"));
    expect(state).toMatchObject({
      version: 2,
      branch: BRANCH,
      findings_count: 0,
      reviewers: ["toolu-review:review"],
      reviewed_files: ["math.test.ts"],
    });

    expect(await refusal(hooks, push)).toBe("allowed");
    expect((await bash(hooks, sb, push)).exitCode).toBe(0);
    expect(remoteHead(sb, remote, BRANCH)).toBe(sb.git("rev-parse", "HEAD").trim());
  });
});

test.concurrent("a failing test run blocks every commit until the tests pass", async () => {
  using sb = createSandbox({ git: true });
  reviewProject(sb);
  sb.write("math.test.ts", FAILING_TEST);
  sb.git("add", "math.test.ts");
  const head = sb.git("rev-parse", "HEAD").trim();
  await withHooks(binding(sb, [], ""), async (hooks) => {
    expect((await bash(hooks, sb, "bun run test")).exitCode).not.toBe(0);
    const denied = await Promise.all(
      ['git commit -m "fix: x"', 'git commit --no-verify -m "fix: x"'].map((commit) =>
        refusal(hooks, commit),
      ),
    );
    for (const reason of denied) expect(reason).toContain("quality gate");
    expect(sb.git("rev-parse", "HEAD").trim()).toBe(head);

    sb.write("math.test.ts", PASSING_TEST.replace("two numbers", "two small numbers"));
    sb.git("add", "math.test.ts");
    expect((await bash(hooks, sb, "bun run test")).exitCode).toBe(0);
    const commit = 'git commit -m "fix: x"';
    expect(await refusal(hooks, commit)).toBe("allowed");
    expect((await bash(hooks, sb, commit)).exitCode).toBe(0);
    expect(sb.git("rev-parse", "HEAD").trim()).not.toBe(head);
  });
});

/** The debug skill's collector, as the agent's bash runs it, over a real failing `bun test`. */
async function debugSummary(sb: Sandbox, repoRootOption: string): Promise<string> {
  let stdout = "";
  await withHooks({ ...binding(sb, [], ""), repoRootOption }, async (hooks) => {
    sb.write("math.test.ts", FAILING_TEST);
    const res = await inShell(sb, debugTestfailCommand(), await bashEnv(hooks, sb));
    expect(res).toMatchObject({ exitCode: 0, stderr: "" });
    stdout = res.stdout;
  });
  return stdout;
}

test.concurrent("the debug helper names the failing test from the clone layout", async () => {
  using sb = createSandbox({ git: true });
  reviewProject(sb);
  const summary = await debugSummary(sb, REPO_ROOT);
  expect(summary).toContain(`  - ${FAILING_NAME}`);
  expect(summary).toMatch(/math\.test\.ts:4:\d+/);
});

test.concurrent("the debug helper names the failing test from the npm layout", async () => {
  using sb = createSandbox({ git: true });
  reviewProject(sb);
  const pkg = join(sb.root, "pkg");
  stagePlugins(join(REPO_ROOT, "plugins"), join(pkg, "plugins"));
  const summary = await debugSummary(sb, pkg);
  expect(summary).toContain(`  - ${FAILING_NAME}`);
  expect(summary).toMatch(/math\.test\.ts:4:\d+/);
});
