/**
 * Live bash oracle (#284 AC-5): the `bash` value recorded in every `live`
 * fixture is what the unmodified shipped libs return today. The suite sources
 * plugins/toolu/hooks/lib/detect.sh and pre-tools/modules/bash-commands.sh and
 * writes nothing under plugins/. Delete it with the bash implementation (#279).
 */
import { expect, test } from "bun:test";
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import {
  bashBatch,
  bashDecide,
  cleanEnv,
  DecideCase,
  initRepo,
  pathWithoutPython,
  readFixture,
  relativeTo,
  scratch,
  type Env,
} from "./parity-helpers.ts";

const Command = z.object({ command: z.string() }).passthrough();
const Git = Command.extend({ fn: z.enum(["is_git_push", "is_git_commit"]), bash: z.boolean() });
const Writes = Command.extend({ fn: z.literal("bash_write_targets"), bash: z.array(z.string()) });
const Decide = DecideCase.extend({
  fn: z.literal("bash_commands_decide"),
  env: z.literal("no-python3").optional(),
  bash: z.string(),
});
const Root = Command.extend({
  fn: z.literal("push_target_root"),
  repos: z.array(z.string()),
  cwd: z.string(),
  env: z.record(z.string(), z.string()).optional(),
  bash: z.string(),
});
const Branch = Command.extend({
  fn: z.literal("push_target_branch"),
  bash: z.object({ attached: z.string(), detached: z.string() }),
});
const Live = z.discriminatedUnion("fn", [Git, Writes, Decide, Root, Branch]);
type LiveCase = z.infer<typeof Live>;

/** Both fixture files as one list of live cases; #283's `kind` is the bash function it exercises. */
function liveCases(): LiveCase[] {
  const bats = z
    .object({ cases: z.array(z.unknown()) })
    .parse(readFixture("bats-parity.json")).cases;
  const issue = z
    .object({ cases: z.array(z.object({ oracle: z.string(), kind: z.string() }).passthrough()) })
    .parse(readFixture("issue-283.json"))
    .cases.filter((c) => c.oracle === "live")
    .map((c) => ({ ...c, fn: c.kind }));
  return [...bats, ...issue].map((c) => Live.parse(c));
}
const cases = liveCases();

function withHome<T>(run: (dir: string, env: Env) => T): T {
  using box = scratch("shell-oracle-");
  mkdirSync(join(box.dir, "home"));
  return run(box.dir, cleanEnv(join(box.dir, "home")));
}

test("the oracle covers both fixture files", () => {
  expect(cases.length).toBeGreaterThanOrEqual(186 + 38);
});

test.concurrent("is_git_push and is_git_commit match the recorded results", () => {
  withHome((_, env) => {
    for (const fn of ["is_git_push", "is_git_commit"] as const) {
      const list = cases.flatMap((c) => (c.fn === fn ? [c] : []));
      const body = `if ${fn} "$f1"; then printf yes; else printf no; fi`;
      const got = bashBatch(
        "detect",
        body,
        list.map((c) => [c.command]),
        env,
      );
      expect(got.map((out) => out === "yes")).toEqual(list.map((c) => c.bash));
    }
  });
});

test.concurrent("bash_write_targets matches the recorded results", () => {
  withHome((_, env) => {
    const list = cases.filter((c) => c.fn === "bash_write_targets");
    const got = bashBatch(
      "detect",
      'bash_write_targets "$f1"',
      list.map((c) => [c.command]),
      env,
    );
    expect(got.map((out) => out.split("\n").filter((line) => line !== ""))).toEqual(
      list.map((c) => c.bash),
    );
  });
});

test.concurrent("bash_commands_decide matches the recorded results, with and without python3", () => {
  withHome((dir, env) => {
    const list = cases.filter((c) => c.fn === "bash_commands_decide");
    const noPython = { ...env, PATH: pathWithoutPython(dir) };
    const got = list.map(
      (c, i) =>
        bashDecide([c], join(dir, `decide-${i}`), c.env === "no-python3" ? noPython : env)[0],
    );
    expect(got).toEqual(list.map((c) => c.bash));
  });
});

for (const c of cases) {
  if (c.fn !== "push_target_root" && c.fn !== "push_target_branch") continue;
  test.concurrent(`${c.fn}: ${JSON.stringify(c.command)}`, () => {
    using layout = scratch("shell-oracle-git-");
    const env = cleanEnv(join(layout.dir, "home"));
    if (c.fn === "push_target_root") {
      for (const repo of c.repos) initRepo(join(layout.dir, repo));
      for (const d of ["outside", "codex-project"])
        mkdirSync(join(layout.dir, d), { recursive: true });
      const command = c.command
        .replaceAll("$MKTEMP", join(layout.dir, "mk"))
        .replaceAll("$TMP", join(layout.dir, "tmp"));
      const extra = Object.fromEntries(
        Object.entries(c.env ?? {}).map(([k, v]) => [k, v.replaceAll("$ROOT", layout.dir)]),
      );
      const [out] = bashBatch(
        "detect",
        'push_target_root "$f1"',
        [[command]],
        { ...env, ...extra },
        join(layout.dir, c.cwd),
      );
      expect(relativeTo(layout.dir, (out ?? "").trim())).toBe(c.bash);
      return;
    }
    const repo = join(layout.dir, "tmp");
    initRepo(repo, "feat/x");
    const command = c.command
      .replaceAll("$MKTEMP", join(layout.dir, "mk"))
      .replaceAll("$TMP", repo);
    const branch = () =>
      (
        bashBatch("detect", 'push_target_branch "$f1" "$f2"', [[command, repo]], env)[0] ?? ""
      ).trim();
    expect(branch()).toBe(c.bash.attached);
    Bun.spawnSync(["git", "-C", repo, "checkout", "-q", "--detach"]);
    expect(branch()).toBe(c.bash.detached);
  });
}
