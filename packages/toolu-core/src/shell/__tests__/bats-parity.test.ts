/**
 * Bash parity (#284 AC-4): every input a bats suite gave the six shipped bash
 * functions (checked in as tooling/fixtures/shell/bats-parity.json) gets the
 * same answer from `@toolu/core/shell`. Push roots and branches are resolved
 * against real git repositories laid out the way the bats suites built them.
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import {
  cleanEnv,
  DecideCase,
  initRepo,
  readFixture,
  relativeTo,
  scratch,
  tsDecide,
  tsIsGit,
  tsPushBranch,
  tsPushRoot,
  tsWriteTargets,
} from "./parity-helpers.ts";

const Base = z.object({ command: z.string(), suites: z.array(z.string()) });
const Case = z.discriminatedUnion("fn", [
  Base.extend({ fn: z.enum(["is_git_push", "is_git_commit"]), bash: z.boolean() }),
  Base.extend({
    fn: z.literal("bash_write_targets"),
    bash: z.array(z.string()),
    expected: z.array(z.string()).optional(),
    note: z.string().optional(),
  }),
  Base.extend({ fn: z.literal("bash_commands_decide"), bash: z.string() }).merge(DecideCase),
  Base.extend({
    fn: z.literal("push_target_root"),
    repos: z.array(z.string()),
    cwd: z.string(),
    env: z.record(z.string(), z.string()).optional(),
    bash: z.string(),
  }),
  Base.extend({
    fn: z.literal("push_target_branch"),
    bash: z.object({ attached: z.string(), detached: z.string() }),
  }),
]);
type BatsCase = z.infer<typeof Case>;
const { cases } = z.object({ cases: z.array(Case) }).parse(readFixture("bats-parity.json"));

test("the harvest covers all six functions from the bats suites", () => {
  expect(new Set(cases.map((c) => c.fn))).toEqual(
    new Set([
      "is_git_push",
      "is_git_commit",
      "bash_write_targets",
      "bash_commands_decide",
      "push_target_root",
      "push_target_branch",
    ]),
  );
  expect(new Set(cases.flatMap((c) => c.suites)).size).toBeGreaterThanOrEqual(15);
});

const sorted = (values: readonly string[]) => [...values].toSorted();

function rootMatches(c: Extract<BatsCase, { fn: "push_target_root" }>): void {
  using layout = scratch("shell-root-");
  for (const repo of c.repos) initRepo(join(layout.dir, repo));
  for (const dir of ["outside", "codex-project"])
    mkdirSync(join(layout.dir, dir), { recursive: true });
  const command = c.command
    .replaceAll("$MKTEMP", join(layout.dir, "mk"))
    .replaceAll("$TMP", join(layout.dir, "tmp"));
  const extra = Object.fromEntries(
    Object.entries(c.env ?? {}).map(([k, v]) => [k, v.replaceAll("$ROOT", layout.dir)]),
  );
  const env = cleanEnv(join(layout.dir, "home"), extra);
  expect(relativeTo(layout.dir, tsPushRoot(command, join(layout.dir, c.cwd), env))).toBe(c.bash);
}

function branchMatches(c: Extract<BatsCase, { fn: "push_target_branch" }>): void {
  using layout = scratch("shell-branch-");
  const repo = join(layout.dir, "tmp");
  initRepo(repo, "feat/x");
  const command = c.command.replaceAll("$MKTEMP", join(layout.dir, "mk")).replaceAll("$TMP", repo);
  const env = cleanEnv(join(layout.dir, "home"));
  expect(tsPushBranch(command, repo, env)).toBe(c.bash.attached);
  Bun.spawnSync(["git", "-C", repo, "checkout", "-q", "--detach"]);
  expect(tsPushBranch(command, repo, env)).toBe(c.bash.detached);
}

for (const c of cases) {
  test.concurrent(`${c.fn}: ${JSON.stringify(c.command)}`, () => {
    switch (c.fn) {
      case "is_git_push":
        expect(tsIsGit(c.command, "push")).toBe(c.bash);
        break;
      case "is_git_commit":
        expect(tsIsGit(c.command, "commit")).toBe(c.bash);
        break;
      case "bash_write_targets":
        expect(sorted(tsWriteTargets(c.command))).toEqual(sorted(c.expected ?? c.bash));
        break;
      case "bash_commands_decide":
        expect(tsDecide(c)).toBe(c.bash);
        break;
      case "push_target_root":
        rootMatches(c);
        break;
      case "push_target_branch":
        branchMatches(c);
        break;
      default:
        throw new Error(`unhandled case ${JSON.stringify(c)}`);
    }
  });
}
