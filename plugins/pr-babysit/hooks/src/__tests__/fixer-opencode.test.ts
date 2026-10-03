import { afterEach, expect, test } from "bun:test";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { dispatchFix, nativeWorktreePath } from "../babysit/fixer-dispatch.ts";
import {
  FIXER_AGENT,
  fixerConfigContent,
  groupAlive,
  opencodeFixerArgs,
  processStart,
  stopGroup,
} from "../babysit/fixer-process.ts";

// OpenCode fixers (#357): config content, argv, real process-group lifecycle,
// and a real dispatch into a native worktree of a sandbox repository.

const root = resolve(import.meta.dir, "../../../../..");
const initial = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/states/toolu-165-initial.json",
);
const KEYS = [
  "PATH",
  "OPENCODE_CONFIG_CONTENT",
  "GIT_AUTHOR_NAME",
  "GIT_AUTHOR_EMAIL",
  "GIT_COMMITTER_NAME",
  "GIT_COMMITTER_EMAIL",
  "GIT_CONFIG_NOSYSTEM",
  "GIT_CONFIG_GLOBAL",
] as const;
const original = Object.fromEntries(KEYS.map((key) => [key, process.env[key]]));
const temps: string[] = [];
const groups: { pid: number; start: string }[] = [];

afterEach(() => {
  for (const { pid, start } of groups.splice(0)) stopGroup(pid, start);
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
  for (const [key, value] of Object.entries(original)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

function temp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  temps.push(dir);
  return dir;
}

function git(cwd: string, ...args: string[]): string {
  const res = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
  return res.stdout.trim();
}

/** A repository with a bare origin holding `main` and the PR branch `feat/fix`. */
function repository(): { dir: string; repo: string } {
  const dir = temp("pr-babysit-opencode-repo-");
  process.env.GIT_AUTHOR_NAME = "Babysit Test";
  process.env.GIT_AUTHOR_EMAIL = "babysit@example.test";
  process.env.GIT_COMMITTER_NAME = "Babysit Test";
  process.env.GIT_COMMITTER_EMAIL = "babysit@example.test";
  process.env.GIT_CONFIG_NOSYSTEM = "1";
  process.env.GIT_CONFIG_GLOBAL = join(dir, "gitconfig");
  writeFileSync(join(dir, "gitconfig"), "");
  const origin = join(dir, "origin.git");
  const repo = join(dir, "repo");
  git(dir, "init", "--quiet", "--bare", "-b", "main", origin);
  git(dir, "init", "--quiet", "-b", "main", repo);
  writeFileSync(join(repo, "sum.ts"), "export const sum = (a: number, b: number) => a - b;\n");
  git(repo, "add", "sum.ts");
  git(repo, "commit", "--quiet", "-m", "init");
  git(repo, "remote", "add", "origin", origin);
  git(repo, "push", "--quiet", "origin", "main");
  git(repo, "checkout", "--quiet", "-b", "feat/fix");
  writeFileSync(join(repo, "sum.test.ts"), "// failing test\n");
  git(repo, "add", "sum.test.ts");
  git(repo, "commit", "--quiet", "-m", "feat: add test");
  git(repo, "push", "--quiet", "origin", "feat/fix");
  return { dir, repo };
}

/** The captured initial state, with one recorded reply that no fixer outcome may touch. */
function slotState(dir: string): string {
  const state = JSON.parse(readFileSync(initial, "utf8"));
  state.actions.replied = { "2300000001": { at: "2026-10-03T12:00:00Z", replyId: 7234 } };
  const path = join(dir, ".opencode/tmp/pr-babysit/falconiere-toolu-165.json");
  mkdirSync(resolve(path, ".."), { recursive: true });
  writeFileSync(path, JSON.stringify(state));
  return path;
}

function opencodePlan(dir: string): { plan: string; items: string } {
  const items = join(dir, "items.json");
  writeFileSync(
    items,
    JSON.stringify({
      round: 1,
      items: [{ id: "ci:test", kind: "ci", path: "sum.ts", task: "Make sum add its arguments." }],
    }),
  );
  const plan = join(dir, "plan.json");
  writeFileSync(
    plan,
    JSON.stringify({
      version: 1,
      dispatch: "herdr",
      unattended: true,
      groups: [
        {
          seq: 1,
          tier: "trivial",
          host: "opencode",
          model: "probe/m",
          effort: null,
          items: ["ci:test"],
        },
      ],
    }),
  );
  return { plan, items };
}

test("fixer config content adds only the fixer agent and keeps every caller key", () => {
  const added = JSON.parse(fixerConfigContent(undefined));
  expect(Object.keys(added)).toEqual(["agent"]);
  expect(added.agent[FIXER_AGENT]).toMatchObject({
    mode: "primary",
    permission: {
      task: "deny",
      bash: {
        gh: "deny",
        "gh *": "deny",
        "git push": "deny",
        "git push *": "deny",
        "git * push": "deny",
        "git * push *": "deny",
      },
    },
  });
  const caller = {
    model: "probe/default",
    permission: { bash: "ask", edit: { "*.lock": "deny" } },
    agent: { build: { model: "probe/build" } },
  };
  const merged = JSON.parse(fixerConfigContent(JSON.stringify(caller)));
  expect(merged.model).toBe("probe/default");
  expect(merged.permission).toEqual(caller.permission);
  expect(merged.agent.build).toEqual({ model: "probe/build" });
  expect(Object.keys(merged.agent)).toEqual(["build", FIXER_AGENT]);
  expect(JSON.parse(fixerConfigContent("  ")).agent[FIXER_AGENT]).toBeDefined();
  for (const bad of ["{", "[]", '"text"', '{"agent": []}'])
    expect(() => fixerConfigContent(bad)).toThrow("OPENCODE_CONFIG_CONTENT");
});

test("opencode run argv names the fixer agent, worktree, approvals, model and variant", () => {
  const run = { worktree: "/w t", prompt: "Read /b.md", model: "probe/m", effort: "high" };
  expect(opencodeFixerArgs({ ...run, unattended: true })).toEqual([
    "run",
    "--format",
    "json",
    "--dir",
    "/w t",
    "--agent",
    FIXER_AGENT,
    "--auto",
    "--model",
    "probe/m",
    "--variant",
    "high",
    "Read /b.md",
  ]);
  expect(opencodeFixerArgs({ ...run, model: null, effort: null, unattended: false })).toEqual([
    "run",
    "--format",
    "json",
    "--dir",
    "/w t",
    "--agent",
    FIXER_AGENT,
    "Read /b.md",
  ]);
});

test("a process group with a backgrounded grandchild stays alive until stopGroup ends all of it", () => {
  const child = spawn("sh", ["-c", "sleep 30 & sleep 30"], { detached: true, stdio: "ignore" });
  const pid = child.pid ?? 0;
  const start = processStart(pid);
  groups.push({ pid, start });
  expect(start).not.toBe("");
  Bun.sleepSync(200);
  expect(groupAlive(pid, start)).toBe(true);
  // A recorded start that differs is a reused pid: never the fixer, never signalled.
  expect(groupAlive(pid, "Thu Jan  1 00:00:00 1970")).toBe(false);
  expect(stopGroup(pid, "Thu Jan  1 00:00:00 1970")).toBe(true);
  expect(groupAlive(pid, start)).toBe(true);
  expect(stopGroup(pid, start)).toBe(true);
  expect(groupAlive(pid, start)).toBe(false);
  const left = spawnSync("ps", ["-A", "-o", "pgid=,stat="], { encoding: "utf8" })
    .stdout.split("\n")
    .filter((line) => line.trim().split(/\s+/)[0] === String(pid) && !/\sZ/.test(line));
  expect(left).toEqual([]);
});

test("dry run of an all-OpenCode plan uses a native worktree and no herdr", () => {
  const dir = temp("pr-babysit-opencode-dry-");
  const stateFile = slotState(dir);
  const before = readFileSync(stateFile, "utf8");
  const { plan, items } = opencodePlan(dir);
  const result = dispatchFix({
    sub: "start",
    stateFile,
    planFile: plan,
    itemsFile: items,
    repoRoot: join(dir, "repo"),
    branch: "feat/fix",
    base: "main",
    dryRun: true,
  });
  const commands = result.commands as string[][];
  expect(commands.some((command) => command[0] === "herdr")).toBe(false);
  expect(
    commands.find((command) => command.includes("worktree") && command.includes("add")),
  ).toEqual([
    "git",
    "-C",
    join(dir, "repo"),
    "worktree",
    "add",
    "--quiet",
    "-b",
    "pr-babysit/falconiere-toolu-165",
    nativeWorktreePath(stateFile),
    "origin/feat/fix",
  ]);
  expect(commands.at(-1)?.slice(0, 8)).toEqual([
    "opencode",
    "run",
    "--format",
    "json",
    "--dir",
    nativeWorktreePath(stateFile),
    "--agent",
    FIXER_AGENT,
  ]);
  expect(readFileSync(stateFile, "utf8")).toBe(before);
});

test("a fixer that cannot start settles agent_start_failed; cleanup removes the native worktree", () => {
  const { dir, repo } = repository();
  const stateFile = slotState(join(repo));
  const { plan, items } = opencodePlan(dir);
  process.env.PATH = "/usr/bin:/bin";
  const ledger = JSON.parse(readFileSync(stateFile, "utf8")).actions;
  const started = dispatchFix({
    sub: "start",
    stateFile,
    planFile: plan,
    itemsFile: items,
    repoRoot: repo,
    branch: "feat/fix",
    base: "main",
  });
  const worktree = nativeWorktreePath(stateFile);
  expect(started).toMatchObject({
    status: "failed",
    reason: "agent_start_failed",
    worktree,
    branch: "pr-babysit/falconiere-toolu-165",
    commits: [],
    groups: [
      {
        seq: 1,
        host: "opencode",
        status: "failed",
        reason: "agent_start_failed",
        error: "opencode is not on PATH",
      },
    ],
  });
  expect(git(worktree, "rev-parse", "HEAD")).toBe(git(repo, "rev-parse", "origin/feat/fix"));
  const state = JSON.parse(readFileSync(stateFile, "utf8"));
  expect(state.actions).toEqual(ledger);
  expect(state.herdrWorktree).toMatchObject({ path: worktree, workspaceId: null, paneId: null });
  expect(dispatchFix({ sub: "wait", stateFile, timeoutSeconds: 1 })).toMatchObject({
    status: "failed",
    reason: "agent_start_failed",
  });

  // OpenCode's own session files are not fixer work; a source file is.
  mkdirSync(join(worktree, ".opencode"), { recursive: true });
  writeFileSync(join(worktree, ".opencode/package.json"), "{}");
  writeFileSync(join(worktree, "extra.ts"), "export {};\n");
  expect(() => dispatchFix({ sub: "cleanup", stateFile })).toThrow("uncommitted changes");
  expect(existsSync(worktree)).toBe(true);
  rmSync(join(worktree, "extra.ts"));
  expect(dispatchFix({ sub: "cleanup", stateFile })).toMatchObject({
    status: "cleaned",
    worktreeRemoved: true,
    branchDeleted: true,
  });
  expect(existsSync(worktree)).toBe(false);
  const after = JSON.parse(readFileSync(stateFile, "utf8"));
  expect(after.fixer).toBeNull();
  expect(after.herdrWorktree).toBeNull();
  expect(after.actions).toEqual(ledger);
});
