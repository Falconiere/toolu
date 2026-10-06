/**
 * #283 fixed (#284 AC-6): every example in the defect list is a named fixture
 * (fixtures/shell/issue-283.json) on which `@toolu/core/shell` gives the
 * correct answer, and the bash result is kept as the known-wrong baseline.
 */
import { expect, test } from "bun:test";
import { join } from "node:path";
import { z } from "zod";
import { commitMessages, gitInvocation, runsGitSubcommand } from "../shell-git.ts";
import { analyzeShell } from "../shell-parse.ts";
import {
  cleanEnv,
  initRepo,
  readFixture,
  relativeTo,
  scratch,
  tsDecide,
  tsIsGit,
  tsPushRoot,
  tsWriteTargets,
} from "./parity-helpers.ts";

const Base = z.object({
  id: z.string().regex(/^283-\d+[a-z]$/),
  item: z.number().int().min(1).max(11),
  title: z.string().min(1),
  command: z.string(),
});
const Live = Base.extend({ oracle: z.literal("live") });
const Recorded = Base.extend({ oracle: z.literal("recorded"), bash: z.string().min(1) });
const Case = z.discriminatedUnion("kind", [
  Live.extend({
    kind: z.enum(["is_git_push", "is_git_commit"]),
    expected: z.boolean(),
    bash: z.boolean(),
  }),
  Live.extend({
    kind: z.literal("bash_write_targets"),
    expected: z.array(z.string()),
    bash: z.array(z.string()),
  }),
  Live.extend({
    kind: z.literal("bash_commands_decide"),
    allow: z.array(z.string()),
    deny: z.array(z.string()),
    env: z.literal("no-python3").optional(),
    expected: z.string(),
    bash: z.string(),
  }),
  Live.extend({
    kind: z.literal("push_target_root"),
    repos: z.array(z.string()),
    cwd: z.string(),
    expected: z.string(),
    bash: z.string(),
  }),
  Recorded.extend({ kind: z.literal("commit_messages"), expected: z.array(z.string()) }),
  Recorded.extend({
    kind: z.literal("commands"),
    expected: z.array(z.object({ argv: z.array(z.string().nullable()), pipeline: z.number() })),
  }),
  Recorded.extend({
    kind: z.literal("exit_proves"),
    expected: z.array(z.tuple([z.string(), z.boolean()])),
  }),
  Recorded.extend({
    kind: z.literal("latency"),
    expected: z.object({ maxMs: z.number(), push: z.boolean() }),
  }),
]);
type Issue283Case = z.infer<typeof Case>;
const { cases } = z.object({ cases: z.array(Case) }).parse(readFixture("issue-283.json"));

test("every #283 item has at least one named fixture", () => {
  expect(new Set(cases.map((c) => c.item))).toEqual(new Set([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]));
  expect(new Set(cases.map((c) => c.id)).size).toBe(cases.length);
});

function rootOf(c: Extract<Issue283Case, { kind: "push_target_root" }>): string {
  using layout = scratch("shell-283-root-");
  for (const repo of c.repos) initRepo(join(layout.dir, repo));
  const command = c.command.replaceAll("$TMP", join(layout.dir, "tmp"));
  const env = cleanEnv(join(layout.dir, "home"));
  return relativeTo(layout.dir, tsPushRoot(command, join(layout.dir, c.cwd), env));
}

function messagesOf(command: string): readonly (string | null)[] {
  const commit = analyzeShell(command)
    .commands.map(gitInvocation)
    .find((git) => git?.subcommand === "commit");
  return commit === undefined ? [] : commitMessages(commit);
}

/** The TypeScript answer for a case, in the shape its `expected` uses. */
function actual(c: Issue283Case): unknown {
  switch (c.kind) {
    case "is_git_push":
    case "is_git_commit":
      return tsIsGit(c.command, c.kind === "is_git_push" ? "push" : "commit");
    case "bash_write_targets":
      return tsWriteTargets(c.command).toSorted();
    case "bash_commands_decide":
      return tsDecide(c);
    case "push_target_root":
      return rootOf(c);
    case "commit_messages":
      return messagesOf(c.command);
    case "commands":
      return analyzeShell(c.command).commands.map((cmd) => ({
        argv: cmd.argv,
        pipeline: cmd.pipeline.index,
      }));
    case "exit_proves":
      return analyzeShell(c.command).commands.map((cmd) => [cmd.argv[0], cmd.exitProves]);
    case "latency": {
      const started = performance.now();
      const push = runsGitSubcommand(analyzeShell(c.command), "push") === "yes";
      const elapsed = performance.now() - started;
      // Within budget reads back as the budget; over it, the diff shows the real time.
      return { maxMs: elapsed <= c.expected.maxMs ? c.expected.maxMs : elapsed, push };
    }
    default:
      throw new Error(`unhandled kind in ${JSON.stringify(c)}`);
  }
}

const expectedOf = (c: Issue283Case): unknown =>
  c.kind === "bash_write_targets" ? c.expected.toSorted() : c.expected;

for (const c of cases) {
  test.concurrent(`${c.id} (item ${c.item}): ${c.title}`, () => {
    expect(actual(c)).toEqual(expectedOf(c));
    // The baseline is the defect: a live bash result must differ from the fix.
    if (c.oracle === "live") expect(c.bash).not.toEqual(c.expected);
  });
}
