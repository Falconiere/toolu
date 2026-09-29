/**
 * Real-session smoke for pr-babysit's herdr fixer dispatch (spec AC-10).
 *
 * Runs the shipped route-fix.sh and dispatch-fix.sh against a live herdr server
 * and a real Claude Code fixer, inside a throwaway local git topology (a bare
 * "origin" plus a clone). Nothing is pushed to GitHub. Proves: start creates the
 * pr-babysit/<slot> herdr worktree and starts the fixer; a second start is
 * refused (fixer_running); wait settles two tier groups in order, `done` with
 * exactly two commits touching only the smoke file; cleanup leaves no worktree,
 * branch or agent behind.
 *
 * Manual runner (needs herdr, claude, jq, git) — like tooling/src/codex-smoke.ts.
 * PB_SMOKE_MODEL / PB_SMOKE_EFFORT pick the fixer model (default haiku / low).
 */
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  rmdirSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { get, list, text } from "./json-path.ts";

const ROOT = realpathSync(resolve(import.meta.dir, "../.."));
const SCRIPTS = join(ROOT, "plugins/pr-babysit/scripts");
const SNAPSHOT = join(SCRIPTS, "__tests__/fixtures/snapshots/toolu-165.json");
const Snapshot = z.looseObject({ pr: z.looseObject({}) });

class SmokeError extends Error {}

function fail(message: string): never {
  throw new SmokeError(message);
}

function step(message: string): void {
  process.stdout.write(`pr-babysit-herdr-smoke: ${message}\n`);
}

type Out = { status: number; stdout: string };

function sh(argv: string[], opts: { cwd?: string; env?: Record<string, string> } = {}): Out {
  const res = spawnSync(argv[0] ?? "", argv.slice(1), {
    cwd: opts.cwd,
    encoding: "utf8",
    env: { ...process.env, ...opts.env },
  });
  if (res.error) throw res.error;
  return { status: res.status ?? 1, stdout: res.stdout };
}

function must(argv: string[], opts: { cwd?: string; env?: Record<string, string> } = {}): string {
  const out = sh(argv, opts);
  if (out.status !== 0) fail(`${argv.join(" ")} exited ${String(out.status)}: ${out.stdout}`);
  return out.stdout;
}

function json(raw: string): unknown {
  try {
    return JSON.parse(raw);
  } catch {
    return fail(`expected JSON, got: ${raw}`);
  }
}

function babysit(script: string, args: string[], env: Record<string, string> = {}): Out {
  return sh(["bash", join(SCRIPTS, script), ...args], { env });
}

/** A throwaway "PR": bare origin, main + feat/smoke, a clone on the PR branch. */
function makeTopology(tmp: string, clone: string): void {
  must(["git", "init", "--quiet", "--bare", "--initial-branch=main", join(tmp, "origin.git")]);
  must(["git", "clone", "--quiet", join(tmp, "origin.git"), clone]);
  const g = (...args: string[]): string => must(["git", "-C", clone, ...args]);
  g("config", "user.email", "smoke@example.invalid");
  g("config", "user.name", "pr-babysit smoke");
  writeFileSync(join(clone, "smoke.txt"), "hello\n");
  g("add", "smoke.txt");
  g("commit", "--quiet", "-m", "chore: seed");
  g("push", "--quiet", "origin", "main");
  g("checkout", "--quiet", "-b", "feat/smoke");
  writeFileSync(join(clone, "smoke.txt"), "hello\nthe PR change\n");
  g("commit", "--quiet", "-am", "feat: smoke change");
  g("push", "--quiet", "-u", "origin", "feat/smoke");
}

const ITEMS = {
  round: 1,
  items: [
    {
      id: "smoke-1",
      kind: "conversation",
      path: "smoke.txt",
      severity: "medium",
      task: "Append exactly one line reading: fixed by pr-babysit smoke — to the end of smoke.txt, then commit only that change with the message: fix(smoke): append line. Change nothing else and run no tests (this file has none).",
    },
    {
      id: "smoke-2",
      kind: "conversation",
      path: "smoke.txt",
      severity: "nit",
      task: "Fix the typo on the first line of smoke.txt: it must read hello world instead of hello. Commit only that change with the message: fix(smoke): typo. Change nothing else and run no tests.",
    },
  ],
};

/** Slot state from a real tick, then two Fix items in two tiers routed to real Claude fixers. */
function route(tmp: string, state: string): void {
  const snap = Snapshot.parse(json(readFileSync(SNAPSHOT, "utf8")));
  const reidentified = {
    ...snap,
    repo: "local/pb-smoke",
    number: 1,
    pr: { ...snap.pr, state: "OPEN" },
  };
  writeFileSync(join(tmp, "snap.json"), JSON.stringify(reidentified));
  const tick = babysit("babysit-tick.sh", [
    "--repo",
    "local/pb-smoke",
    "--pr",
    "1",
    "--state-file",
    state,
    "--snapshot-in",
    join(tmp, "snap.json"),
  ]);
  if (tick.status !== 0) fail(`babysit-tick failed: ${tick.stdout}`);
  const slot = get(json(readFileSync(state, "utf8")), "slot");
  if (slot !== "local-pb-smoke-1") fail(`unexpected slot ${String(slot)}`);
  writeFileSync(join(tmp, "items.json"), JSON.stringify(ITEMS));
  const tier = {
    model: process.env["PB_SMOKE_MODEL"] ?? "haiku",
    effort: process.env["PB_SMOKE_EFFORT"] ?? "low",
  };
  mkdirSync(join(tmp, "cfg"), { recursive: true });
  const config = {
    prBabysit: { hosts: ["claude"], jev: false, routing: { claude: [tier, tier, tier, tier] } },
  };
  writeFileSync(join(tmp, "cfg/toolu.config.json"), JSON.stringify(config));
  const cfgEnv = { TOOLU_CONFIG_DIR: join(tmp, "cfg"), TOOLU_PROJECT_DIR: join(tmp, "cfg") };
  const routed = babysit(
    "route-fix.sh",
    ["--items", join(tmp, "items.json"), "--host", "claude", "--no-jev"],
    cfgEnv,
  );
  writeFileSync(join(tmp, "route.json"), routed.stdout);
  const plan = json(routed.stdout);
  if (get(plan, "dispatch") !== "herdr") fail(`route did not dispatch to herdr: ${routed.stdout}`);
  const tiers = list(get(plan, "groups")).map((g) => get(g, "tier"));
  if (JSON.stringify(tiers) !== '["standard","trivial"]')
    fail(`expected two tier groups: ${routed.stdout}`);
  const summary = list(get(plan, "groups")).map((g) => ({
    tier: get(g, "tier"),
    host: get(g, "host"),
    model: get(g, "model"),
    effort: get(g, "effort"),
  }));
  step(`route: ${JSON.stringify(summary)}`);
}

function startArgs(tmp: string, state: string, clone: string): string[] {
  return [
    "start",
    "--state-file",
    state,
    "--plan",
    join(tmp, "route.json"),
    "--items",
    join(tmp, "items.json"),
    "--repo-root",
    clone,
    "--branch",
    "feat/smoke",
    "--base",
    "main",
  ];
}

/** start → refused second start → bounded wait until the fixer settles. */
function dispatch(
  tmp: string,
  state: string,
  clone: string,
): { worktree: string; agent: string; out: unknown } {
  const started = babysit("dispatch-fix.sh", startArgs(tmp, state, clone));
  const start = json(started.stdout);
  if (started.status !== 0 || get(start, "status") !== "running")
    fail(`start status is not running: ${started.stdout}`);
  const worktree = String(get(start, "worktree"));
  const agent = String(get(start, "groups", 0, "agent"));
  step(
    `start: ${JSON.stringify({ status: get(start, "status"), worktree, branch: get(start, "branch"), agent })}`,
  );
  if (!existsSync(worktree)) fail(`worktree path missing: ${worktree}`);
  if (
    must(["git", "-C", worktree, "branch", "--show-current"]).trim() !==
    "pr-babysit/local-pb-smoke-1"
  ) {
    fail("worktree is not on pr-babysit/local-pb-smoke-1");
  }
  const again = babysit("dispatch-fix.sh", startArgs(tmp, state, clone));
  if (again.status !== 3 || get(json(again.stdout), "errors", 0, "code") !== "fixer_running") {
    fail(`second start was not refused: rc=${String(again.status)} ${again.stdout}`);
  }
  step("second start: fixer_running (exit 3)");
  let out: unknown = null;
  for (let i = 0; i < 6; i += 1) {
    out = json(
      babysit("dispatch-fix.sh", ["wait", "--state-file", state, "--timeout-seconds", "150"])
        .stdout,
    );
    step(
      `wait: ${JSON.stringify({ status: get(out, "status"), reason: get(out, "reason"), commits: get(out, "commits") })}`,
    );
    if (get(out, "status") !== "running") break;
  }
  return { worktree, agent, out };
}

function verifyFixes(worktree: string, out: unknown): void {
  if (get(out, "status") !== "done") fail(`fixer did not finish done: ${JSON.stringify(out)}`);
  const groups = list(get(out, "groups")).map((g) => get(g, "status"));
  if (JSON.stringify(groups) !== '["done","done"]')
    fail(`both groups should be done: ${JSON.stringify(out)}`);
  if (list(get(out, "commits")).length !== 2)
    fail(`expected exactly two fixer commits: ${JSON.stringify(out)}`);
  const changed = [
    ...new Set(
      must([
        "git",
        "-C",
        worktree,
        "log",
        "--name-only",
        "--format=",
        "refs/remotes/origin/feat/smoke..HEAD",
      ])
        .split("\n")
        .filter(Boolean),
    ),
  ];
  if (changed.join() !== "smoke.txt") fail(`fixer commits touched: ${changed.join(" ")}`);
  const lines = readFileSync(join(worktree, "smoke.txt"), "utf8").trimEnd().split("\n");
  if (!(lines.at(-1) ?? "").includes("fixed by pr-babysit smoke"))
    fail("smoke.txt does not end with the first fixer's line");
  if (lines[0] !== "hello world") fail("the second fixer did not fix the first line");
}

function verifyCleanup(state: string, clone: string, worktree: string, agent: string): void {
  must(["git", "-C", worktree, "push", "--quiet", "origin", "HEAD:feat/smoke"]);
  const cleaned = babysit("dispatch-fix.sh", ["cleanup", "--state-file", state]);
  if (cleaned.status !== 0) fail(`cleanup failed: ${cleaned.stdout}`);
  step(`cleanup: ${cleaned.stdout.trim()}`);
  const result = json(cleaned.stdout);
  if (get(result, "worktreeRemoved") !== true || get(result, "branchDeleted") !== true)
    fail(`cleanup left something behind: ${cleaned.stdout}`);
  if (existsSync(worktree)) fail(`worktree directory still exists: ${worktree}`);
  try {
    rmdirSync(dirname(worktree)); // herdr's per-repo parent, empty for this throwaway repo
  } catch {
    // Not empty or already gone: nothing of ours to remove.
  }
  if (must(["git", "-C", clone, "worktree", "list"]).trim().split("\n").length !== 1)
    fail("git still lists a linked worktree");
  if (must(["git", "-C", clone, "branch", "--list", "pr-babysit/*"]).trim() !== "")
    fail("a pr-babysit/* branch is left");
  const worktrees = list(
    get(json(must(["herdr", "worktree", "list", "--cwd", clone])), "result", "worktrees"),
  );
  if (worktrees.some((w) => text(get(w, "branch")).startsWith("pr-babysit/")))
    fail("herdr still lists a pr-babysit worktree");
  const agents = list(get(json(must(["herdr", "agent", "list"])), "result", "agents"));
  if (agents.some((a) => get(a, "name") === agent)) fail(`fixer agent ${agent} is still live`);
  const final = json(readFileSync(state, "utf8"));
  if (get(final, "herdrWorktree") !== null || get(final, "fixer") !== null)
    fail("state still records the worktree or fixer");
}

/** Best effort: cleanup a fixer we started, close the herdr workspace opened for the clone, drop the tree. */
function finish(tmp: string, state: string): void {
  if (
    existsSync(state) &&
    typeof get(json(readFileSync(state, "utf8")), "herdrWorktree") === "object"
  ) {
    babysit("dispatch-fix.sh", ["cleanup", "--state-file", state]);
  }
  const listed = sh(["herdr", "workspace", "list"]);
  if (listed.status === 0) {
    for (const ws of list(get(json(listed.stdout), "result", "workspaces"))) {
      if (text(get(ws, "worktree", "repo_root")).startsWith(tmp))
        sh(["herdr", "workspace", "close", String(get(ws, "workspace_id"))]);
    }
  }
  rmSync(tmp, { recursive: true, force: true });
}

function main(): number {
  for (const tool of ["herdr", "claude", "jq", "git"]) {
    if (Bun.which(tool) === null) {
      console.error(`pr-babysit-herdr-smoke: FAIL: ${tool} is required`);
      return 1;
    }
  }
  if (sh(["herdr", "workspace", "list"]).status !== 0) {
    console.error("pr-babysit-herdr-smoke: FAIL: herdr server is not reachable");
    return 1;
  }
  const tmp = realpathSync(mkdtempSync(join(tmpdir(), "pb-herdr-smoke.")));
  const state = join(tmp, "pr-babysit-local-pb-smoke-1.json");
  const clone = join(tmp, "pb-smoke");
  try {
    makeTopology(tmp, clone);
    route(tmp, state);
    const { worktree, agent, out } = dispatch(tmp, state, clone);
    verifyFixes(worktree, out);
    verifyCleanup(state, clone, worktree, agent);
    step("PASS");
    return 0;
  } catch (err: unknown) {
    if (!(err instanceof SmokeError)) throw err;
    console.error(`pr-babysit-herdr-smoke: FAIL: ${err.message}`);
    return 1;
  } finally {
    finish(tmp, state);
  }
}

if (import.meta.main) process.exitCode = main();
