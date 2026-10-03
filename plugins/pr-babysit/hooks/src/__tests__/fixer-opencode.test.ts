import { expect, test } from "bun:test";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fixerOutcome, nativeWorktreePath } from "../babysit/fixer-dispatch.ts";
import {
  FIXER_AGENT,
  fixerConfigContent,
  fixerEnv,
  groupAlive,
  hostErrors,
  logTail,
  opencodeFixerArgs,
  processStart,
  stopGroup,
} from "../babysit/fixer-process.ts";

// OpenCode fixers (#357): config content, argv and environment, real
// process-group lifecycle, log reading on a captured fixer log, and the shipped
// dispatcher bundle against a sandbox repository. Nothing writes process.env.

const root = resolve(import.meta.dir, "../../../../..");
const dispatchBundle = join(root, "plugins/pr-babysit/hooks/dist/babysit-dispatch-fix.js");
const initial = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/states/toolu-165-initial.json",
);
const capturedLog = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/opencode/fixer-r1g1.log",
);
const HAS_GH = Bun.which("gh") !== null;

type Sandbox = { dir: string; env: Record<string, string>; [Symbol.dispose](): void };

/** A temp dir and an env with a git identity and no user or system git config. */
function sandbox(prefix: string): Sandbox {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  writeFileSync(join(dir, "gitconfig"), "");
  return {
    dir,
    env: {
      HOME: dir,
      PATH: "/usr/bin:/bin",
      GIT_AUTHOR_NAME: "Babysit Test",
      GIT_AUTHOR_EMAIL: "babysit@example.test",
      GIT_COMMITTER_NAME: "Babysit Test",
      GIT_COMMITTER_EMAIL: "babysit@example.test",
      GIT_CONFIG_NOSYSTEM: "1",
      GIT_CONFIG_GLOBAL: join(dir, "gitconfig"),
    },
    [Symbol.dispose]: () => rmSync(dir, { recursive: true, force: true }),
  };
}

function git(
  cwd: string,
  env: NodeJS.ProcessEnv,
  ...args: string[]
): { status: number | null; out: string; err: string } {
  const res = spawnSync("git", args, { cwd, env, encoding: "utf8" });
  return { status: res.status, out: res.stdout.trim(), err: res.stderr };
}

function gitOk(cwd: string, env: NodeJS.ProcessEnv, ...args: string[]): string {
  const res = git(cwd, env, ...args);
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.err}`);
  return res.out;
}

/** A repository with a bare origin holding `main` and the PR branch `feat/fix`. */
function repository(sb: Sandbox): { repo: string; origin: string } {
  const origin = join(sb.dir, "origin.git");
  const repo = join(sb.dir, "repo");
  gitOk(sb.dir, sb.env, "init", "--quiet", "--bare", "-b", "main", origin);
  gitOk(sb.dir, sb.env, "init", "--quiet", "-b", "main", repo);
  writeFileSync(join(repo, "sum.ts"), "export const sum = (a: number, b: number) => a - b;\n");
  gitOk(repo, sb.env, "add", "sum.ts");
  gitOk(repo, sb.env, "commit", "--quiet", "-m", "init");
  gitOk(repo, sb.env, "remote", "add", "origin", origin);
  gitOk(repo, sb.env, "push", "--quiet", "origin", "main");
  gitOk(repo, sb.env, "checkout", "--quiet", "-b", "feat/fix");
  writeFileSync(join(repo, "sum.test.ts"), "// failing test\n");
  gitOk(repo, sb.env, "add", "sum.test.ts");
  gitOk(repo, sb.env, "commit", "--quiet", "-m", "feat: add test");
  gitOk(repo, sb.env, "push", "--quiet", "origin", "feat/fix");
  return { repo, origin };
}

/** The captured initial state at the OpenCode slot path, with one recorded reply. */
function slotState(project: string): string {
  const state = JSON.parse(readFileSync(initial, "utf8"));
  state.actions.replied = { "2300000001": { at: "2026-10-03T12:00:00Z", replyId: 7234 } };
  const path = join(project, ".opencode/tmp/pr-babysit/falconiere-toolu-165.json");
  mkdirSync(dirname(path), { recursive: true });
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

/** The shipped `babysit-dispatch-fix.js` with exactly `env`; its exit and parsed stdout. */
function dispatch(
  env: NodeJS.ProcessEnv,
  args: string[],
): { status: number | null; out: Record<string, unknown> } {
  const res = spawnSync(process.execPath, [dispatchBundle, ...args], { env, encoding: "utf8" });
  const out: Record<string, unknown> = JSON.parse(res.stdout);
  return { status: res.status, out };
}

function startArgs(stateFile: string, plan: string, items: string, repo: string): string[] {
  return [
    "start",
    "--state-file",
    stateFile,
    "--plan",
    plan,
    "--items",
    items,
    "--repo-root",
    repo,
    "--branch",
    "feat/fix",
    "--base",
    "main",
  ];
}

test.concurrent("fixer config content adds only the fixer agent and keeps every caller key", () => {
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

test.concurrent("opencode run argv names the fixer agent, worktree, approvals, model and variant", () => {
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

test.concurrent("the fixer env drops GitHub tokens, keeps caller git config and refuses a bad count", () => {
  const env = fixerEnv(
    {
      GH_TOKEN: "t1",
      GITHUB_TOKEN: "t2",
      GH_ENTERPRISE_TOKEN: "t3",
      GITHUB_ENTERPRISE_TOKEN: "t4",
      GIT_CONFIG_COUNT: "1",
      GIT_CONFIG_KEY_0: "user.name",
      GIT_CONFIG_VALUE_0: "Caller",
      KEEP: "yes",
    },
    "/w",
    "/gh",
  );
  for (const key of ["GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN"])
    expect(env[key]).toBeUndefined();
  expect(env).toMatchObject({
    KEEP: "yes",
    PWD: "/w",
    GH_CONFIG_DIR: "/gh",
    GIT_TERMINAL_PROMPT: "0",
    GIT_CONFIG_KEY_0: "user.name",
    GIT_CONFIG_VALUE_0: "Caller",
    GIT_CONFIG_KEY_1: "credential.helper",
    GIT_CONFIG_VALUE_1: "",
    GIT_CONFIG_KEY_2: "url.pr-babysit-fixer-no-push://.pushInsteadOf",
    GIT_CONFIG_VALUE_2: "/",
    GIT_CONFIG_COUNT: "9",
  });
  expect(JSON.parse(env.OPENCODE_CONFIG_CONTENT ?? "{}").agent[FIXER_AGENT]).toBeDefined();
  expect(() => fixerEnv({ GIT_CONFIG_COUNT: "two" }, "/w", "/gh")).toThrow("GIT_CONFIG_COUNT");
});

test.concurrent("under the fixer env git cannot push in any command form and has no credential helper", () => {
  using sb = sandbox("pr-babysit-nopush-");
  const { repo, origin } = repository(sb);
  writeFileSync(join(repo, "sum.ts"), "export const sum = (a: number, b: number) => a + b;\n");
  gitOk(repo, sb.env, "commit", "--quiet", "-am", "fix: add");
  const before = gitOk(origin, sb.env, "rev-parse", "feat/fix");
  const env = fixerEnv(
    {
      ...sb.env,
      GIT_CONFIG_COUNT: "1",
      GIT_CONFIG_KEY_0: "credential.helper",
      GIT_CONFIG_VALUE_0: "store",
    },
    repo,
    join(sb.dir, "gh"),
  );
  for (const command of [
    "git push origin HEAD:feat/fix",
    `env git -C '${repo}' push origin HEAD:feat/fix`,
    `/usr/bin/git push '${origin}' HEAD:feat/fix`,
  ]) {
    const res = spawnSync("sh", ["-c", command], { cwd: repo, env, encoding: "utf8" });
    expect(res.status, command).not.toBe(0);
    expect(res.stderr, command).toContain("pr-babysit-fixer-no-push");
  }
  expect(gitOk(origin, sb.env, "rev-parse", "feat/fix")).toBe(before);
  // The empty value comes last: it resets the helper list, so no stored credential is used.
  const helpers = spawnSync("git", ["config", "--get-all", "credential.helper"], {
    cwd: repo,
    env,
    encoding: "utf8",
  });
  expect(helpers.stdout).toBe("store\n\n");
  expect(gitOk(repo, env, "rev-parse", "HEAD")).not.toBe(before);
});

test.concurrent.skipIf(!HAS_GH)(
  "under the fixer env gh has no login, even with a token exported",
  () => {
    using sb = sandbox("pr-babysit-nogh-");
    const env = fixerEnv(
      { ...sb.env, PATH: process.env.PATH ?? "", GH_TOKEN: "ghp_babysit_fixture" },
      sb.dir,
      join(sb.dir, "gh"),
    );
    const res = spawnSync("gh", ["auth", "status"], { env, encoding: "utf8" });
    expect(res.status).not.toBe(0);
    expect(`${res.stdout}${res.stderr}`).toMatch(/not logged in/i);
    expect(`${res.stdout}${res.stderr}`).not.toContain("ghp_babysit_fixture");
  },
);

/** Live (non-zombie) pids among `pids`. */
function live(pids: readonly number[]): number[] {
  const table = spawnSync("ps", ["-A", "-o", "pid=,stat="], { encoding: "utf8" }).stdout;
  const alive = new Set(
    table
      .split("\n")
      .map((line) => line.trim().split(/\s+/))
      .filter(([, stat]) => stat !== undefined && !stat.startsWith("Z"))
      .map(([pid]) => Number(pid)),
  );
  return pids.filter((pid) => alive.has(pid));
}

/** Children of `parent` that left its process group; waits up to 10 s for `count` of them. */
function movedChildren(parent: number, count: number): number[] {
  for (let i = 0; i < 100; i += 1) {
    const moved = spawnSync("ps", ["-A", "-o", "pid=,ppid=,pgid="], { encoding: "utf8" })
      .stdout.split("\n")
      .map((line) => line.trim().split(/\s+/).map(Number))
      .filter(([, ppid, pgid]) => ppid === parent && pgid !== parent)
      .map(([child]) => child ?? 0);
    if (moved.length >= count) return moved;
    Bun.sleepSync(100);
  }
  return [];
}

test.concurrent("stopGroup ends the group and a descendant that moved to its own process group", () => {
  // Like OpenCode's bash tool: one grandchild stays in the group, one leaves it.
  const child = spawn("sh", ["-c", "sleep 30 & perl -e 'setpgrp(0, 0); sleep 30' & sleep 30"], {
    detached: true,
    stdio: "ignore",
  });
  const pid = child.pid ?? 0;
  const start = processStart(pid);
  try {
    expect(start).not.toBe("");
    const moved = movedChildren(pid, 1);
    expect(moved).toHaveLength(1);
    expect(groupAlive(pid, start)).toBe(true);
    // A recorded start that differs is a reused pid: never the fixer, never signalled.
    expect(groupAlive(pid, "Thu Jan  1 00:00:00 1970")).toBe(false);
    expect(stopGroup(pid, "Thu Jan  1 00:00:00 1970")).toBe(true);
    expect(groupAlive(pid, start)).toBe(true);
    expect(stopGroup(pid, start)).toBe(true);
    expect(groupAlive(pid, start)).toBe(false);
    expect(live(moved)).toEqual([]);
  } finally {
    stopGroup(pid, start);
  }
});

test.concurrent("limit detection reads only the host's errors in a captured fixer log", () => {
  const tail = readFileSync(capturedLog, "utf8");
  expect(hostErrors(tail).split("\n")).toEqual([
    'timestamp=2026-10-03T20:29:09.660Z level=ERROR run=dddd81cc message="cli process failed" cause="Cause([Fail(~effect/cli/CliError/ShowHelp: Help requested)])" role=cli',
  ]);
  // A tool result and the routine log that mention a rate limit are not a provider limit.
  const noisy = `${tail}{"type":"tool_use","part":{"tool":"bash","state":{"status":"completed","output":"429 Too Many Requests: rate limit"}}}\ntimestamp=2026-10-03T20:29:40.000Z level=INFO message=evaluated pattern="grep -r 'rate limit' src"\n`;
  expect(fixerOutcome("/nonexistent/report.json", hostErrors(noisy))).toBe("no_report");
  const limited = `${tail}{"type":"error","error":{"name":"APIError","data":{"message":"429 Too Many Requests"}}}\n`;
  expect(fixerOutcome("/nonexistent/report.json", hostErrors(limited))).toBe("host_limited");
});

test.concurrent("logTail reads at most the end of a large log and never a cut-off line", () => {
  using sb = sandbox("pr-babysit-tail-");
  const log = join(sb.dir, "fixer.log");
  const lines = Array.from({ length: 5000 }, (_, i) => `line ${i} ${"x".repeat(40)}`);
  writeFileSync(log, `${lines.join("\n")}\n`);
  expect(logTail(log, 3)).toBe(`${lines.slice(-2).join("\n")}\n`);
  expect(logTail(log, 40).split("\n")[0]).toBe(lines.at(-39));
  expect(logTail(join(sb.dir, "missing.log"))).toBe("");
});

test.concurrent("dry run of an all-OpenCode plan uses a native worktree and no herdr", () => {
  using sb = sandbox("pr-babysit-opencode-dry-");
  const stateFile = slotState(sb.dir);
  const before = readFileSync(stateFile, "utf8");
  const { plan, items } = opencodePlan(sb.dir);
  const { status, out } = dispatch(sb.env, [
    ...startArgs(stateFile, plan, items, join(sb.dir, "repo")),
    "--dry-run",
  ]);
  expect(status).toBe(0);
  const commands = out.commands as string[][];
  expect(commands.some((command) => command[0] === "herdr")).toBe(false);
  expect(commands.find((command) => command.includes("add"))).toEqual([
    "git",
    "-C",
    join(sb.dir, "repo"),
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

test.concurrent("a bad OPENCODE_CONFIG_CONTENT is refused before any worktree or fixer record", () => {
  using sb = sandbox("pr-babysit-badcfg-");
  const { repo } = repository(sb);
  const stateFile = slotState(repo);
  const before = readFileSync(stateFile, "utf8");
  const { plan, items } = opencodePlan(sb.dir);
  const { status, out } = dispatch(
    { ...sb.env, OPENCODE_CONFIG_CONTENT: "[1]" },
    startArgs(stateFile, plan, items, repo),
  );
  expect(status).toBe(3);
  expect(JSON.stringify(out)).toContain("config_invalid");
  expect(existsSync(nativeWorktreePath(stateFile))).toBe(false);
  expect(readFileSync(stateFile, "utf8")).toBe(before);
});

test.concurrent("a fixer that cannot start settles agent_start_failed; cleanup removes the native worktree", () => {
  using sb = sandbox("pr-babysit-opencode-repo-");
  const { repo } = repository(sb);
  const stateFile = slotState(repo);
  const { plan, items } = opencodePlan(sb.dir);
  const ledger = JSON.parse(readFileSync(stateFile, "utf8")).actions;
  const started = dispatch(sb.env, startArgs(stateFile, plan, items, repo));
  const worktree = nativeWorktreePath(stateFile);
  expect(started.status).toBe(0);
  expect(started.out).toMatchObject({
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
  expect(gitOk(worktree, sb.env, "rev-parse", "HEAD")).toBe(
    gitOk(repo, sb.env, "rev-parse", "origin/feat/fix"),
  );
  const state = JSON.parse(readFileSync(stateFile, "utf8"));
  expect(state.actions).toEqual(ledger);
  expect(state.herdrWorktree).toMatchObject({ path: worktree, workspaceId: null, paneId: null });
  expect(
    dispatch(sb.env, ["wait", "--state-file", stateFile, "--timeout-seconds", "1"]).out,
  ).toMatchObject({ status: "failed", reason: "agent_start_failed" });

  // OpenCode's own session files are not fixer work; a source file is.
  mkdirSync(join(worktree, ".opencode"), { recursive: true });
  writeFileSync(join(worktree, ".opencode/package.json"), "{}");
  writeFileSync(join(worktree, "extra.ts"), "export {};\n");
  const dirty = dispatch(sb.env, ["cleanup", "--state-file", stateFile]);
  expect(dirty.status).toBe(3);
  expect(JSON.stringify(dirty.out)).toContain("worktree_dirty");
  expect(existsSync(worktree)).toBe(true);
  rmSync(join(worktree, "extra.ts"));
  expect(dispatch(sb.env, ["cleanup", "--state-file", stateFile]).out).toMatchObject({
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
