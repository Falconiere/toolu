/**
 * Live epic worker (#356, AC-3 and AC-9), reported apart from the hermetic
 * suites. `TOOLU_LIVE_OPENCODE=1` runs the pinned host as an epic worker in a
 * linked worktree of an isolated fixture repo, with the launcher's own flags,
 * start prompt, brief and `info/exclude`, and toolu loaded from the global
 * config. The worker reports, loads delivery, delegates through `task`, edits
 * and is killed mid-turn. A checkpoint keeps the edit; `--continue` resumes the
 * same session through commit, review, push and a ready report the watcher sees.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { childEnv, run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { readJson as readEpicJson } from "../../../../../plugins/epic-orchestrator/scripts/common.ts";
import {
  agentArgs,
  opencodeVersionProblem,
} from "../../../../../plugins/epic-orchestrator/scripts/hosts.ts";
import {
  START_PROMPT,
  findIssue,
  renderBrief,
} from "../../../../../plugins/epic-orchestrator/scripts/launch-issue.ts";
import { excludeOpencodeState } from "../../../../../plugins/epic-orchestrator/scripts/opencode-worker.ts";
import {
  RUN_TIMEOUT_MS,
  runHost,
  toolStates,
} from "../../../../../tooling/src/opencode-host/host-run.ts";
import {
  hostCacheDir,
  resolveHostBinary,
} from "../../../../../tooling/src/opencode-host/install.ts";
import type { Scripts } from "../../../../../tooling/src/opencode-host/provider.ts";
import { contractPaths } from "../../../../../tooling/src/opencode-host/results.ts";
import { allRequestText } from "../../../../../tooling/src/opencode-host/scenario.ts";
import { ROOT } from "../../../../../tooling/src/opencode-host/scenarios-entry.ts";
import { PinSchema, readJson } from "../../../../../tooling/src/opencode-host/schema.ts";
import {
  openSession,
  type ProbeSession,
} from "../../../../../tooling/src/opencode-host/session.ts";
import { writeStateCommand } from "./core-fixtures.ts";

type Graph = Parameters<typeof findIssue>[0];

const SCRIPTS = join(ROOT, "plugins/epic-orchestrator/scripts");
const GRAPH = ((): Graph => {
  const graph = readEpicJson<Graph | null>(join(SCRIPTS, "fixtures/epic248-graph.json"), null);
  if (graph === null) throw new Error("missing the epic248 graph fixture");
  return graph;
})();
const ISSUE = findIssue(GRAPH, "Falconiere/comemory#255");
const KEY = "comemory-255";
const MODEL = "probe/scripted";
const REPORT = `bun "${join(SCRIPTS, "report.ts")}"`;
const TOOLU_RUNTIME = /\.opencode\/(toolu\/state|tmp)\//;
const ARGS = { key: KEY, model: MODEL, bypass: true, permissionMode: "default" };

const Event = z.looseObject({ sessionID: z.string().optional() });
const Status = z.looseObject({ phase: z.string(), pr: z.number().nullable() });

function layout(root: string) {
  const state = join(root, "state");
  return {
    state,
    worktree: join(root, "worktree"),
    remote: join(root, "remote.git"),
    status: join(state, "status", `${KEY}.json`),
    brief: join(state, "briefs", `${KEY}.md`),
    killPoint: join(root, "kill-point"),
  };
}

const bash = (command: string) => ({ tool: "bash", args: { command, description: "epic worker" } });

function workerScripts(project: string): Scripts {
  const p = layout(dirname(project));
  return {
    "epic.worker": [
      { tool: "read", args: { filePath: p.brief } },
      bash(`${REPORT} "${p.status}" execution`),
      { tool: "skill", args: { name: "delivery-flow-delivery-flow" } },
      {
        tool: "task",
        args: {
          description: "inspect",
          prompt: "List the files in the worktree.",
          subagent_type: "toolu-quick-task",
        },
      },
      { tool: "write", args: { filePath: join(p.worktree, "CHANGE.md"), content: "Bounded.\n" } },
      // Holds the turn open until the host dies, then exits with it.
      bash(`touch "${p.killPoint}"; while kill -0 $PPID 2>/dev/null; do sleep 1; done`),
    ],
    "epic.resume": [
      bash('git add CHANGE.md && git commit -q -m "feat: bounded change"'),
      bash(writeStateCommand(0)),
      bash(`git push -q -u origin ${ISSUE.branch}`),
      bash(`${REPORT} "${p.status}" ready --pr 1`),
    ],
  };
}

function write(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

/** A repo with a bare remote, a committed selection, the worker's linked worktree and epic state. */
async function epicFixture(session: ProbeSession): Promise<void> {
  const { sb } = session;
  const p = layout(sb.root);
  const hostConfig: unknown = JSON.parse(sb.read("opencode.json"));
  rmSync(join(sb.project, "opencode.json"));
  write(
    join(sb.home, ".config/opencode/opencode.json"),
    JSON.stringify({
      ...z.looseObject({}).parse(hostConfig),
      plugin: [[pathToFileURL(join(ROOT, "tools/toolu-opencode")).href, { repoRoot: ROOT }]],
    }),
  );
  sb.write(".gitignore", "node_modules/\n");
  sb.write(".opencode/toolu/plugins.json", { version: 1, enabled: ["epic-orchestrator"] });
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "chore: epic fixture");
  sb.git("init", "-q", "--bare", p.remote);
  sb.git("remote", "add", "origin", p.remote);
  sb.git("push", "-q", "origin", "main");
  sb.git("worktree", "add", "-q", "-b", ISSUE.branch, p.worktree, "main");
  await excludeOpencodeState(p.worktree);
  const record = { ref: ISSUE.ref, kind: "opencode", stage: "running", agent: KEY };
  write(
    join(p.state, "issues", `${KEY}.json`),
    JSON.stringify({ ...record, worktree: p.worktree, branch: ISSUE.branch }),
  );
  const paths = { worktree: p.worktree, status: p.status, brief: p.brief };
  write(p.brief, renderBrief(GRAPH, ISSUE, paths, "main", "opencode"));
  session.env.TOOLU_BUN = process.execPath;
}

/** Poll for `file` while `proc` runs, up to `deadline`. */
async function reached(file: string, proc: Bun.Subprocess, deadline: number): Promise<boolean> {
  if (existsSync(file)) return true;
  if (proc.exitCode !== null || Date.now() >= deadline) return false;
  await Bun.sleep(250);
  return reached(file, proc, deadline);
}

/** `opencode run` in `cwd`, killed with its process group once the turn reaches `killPoint`. */
async function runUntilKilled(bin: string, session: ProbeSession, args: string[], cwd: string) {
  const stdout = session.outside("killed.stdout");
  const proc = Bun.spawn([bin, "run", "--format", "json", ...args], {
    cwd,
    env: childEnv({ ...session.env, PWD: cwd }),
    stdin: "ignore",
    stdout: Bun.file(stdout),
    stderr: Bun.file(session.outside("killed.stderr")),
    detached: true,
  });
  const atKillPoint = await reached(
    layout(session.sb.root).killPoint,
    proc,
    Date.now() + RUN_TIMEOUT_MS,
  );
  if (proc.exitCode === null) process.kill(-proc.pid, "SIGKILL");
  const exitCode = await proc.exited;
  const events = readFileSync(stdout, "utf8")
    .split("\n")
    .flatMap((line) => {
      const parsed = Event.safeParse(line.startsWith("{") ? JSON.parse(line) : null);
      return parsed.success ? [parsed.data] : [];
    });
  return {
    atKillPoint,
    exitCode,
    events,
    stderr: readFileSync(session.outside("killed.stderr"), "utf8"),
  };
}

const sessionIds = (events: Array<z.infer<typeof Event>>) => [
  ...new Set(events.flatMap((e) => (e.sessionID === undefined ? [] : [e.sessionID]))),
];

const statusOf = (file: string) => Status.parse(JSON.parse(readFileSync(file, "utf8")));

/** The first run: start prompt, delivery, one `task`, an edit, then a kill mid-turn. */
async function killMidTurn(bin: string, session: ProbeSession) {
  const p = layout(session.sb.root);
  const prompt = `${START_PROMPT.replace("{brief}", p.brief)} PROBE:epic.worker`;
  const killed = await runUntilKilled(
    bin,
    session,
    [...agentArgs("opencode", { ...ARGS, resume: false }), prompt],
    p.worktree,
  );
  expect(killed.atKillPoint, killed.stderr.slice(-4000)).toBe(true);
  expect(killed.exitCode).not.toBe(0);
  expect(statusOf(p.status).phase).toBe("execution");
  const states = toolStates({ exitCode: killed.exitCode, events: killed.events, stderr: "" });
  expect(states.slice(0, 5).map(({ tool, status }) => `${tool}:${status}`)).toEqual([
    "read:completed",
    "bash:completed",
    "skill:completed",
    "task:completed",
    "write:completed",
  ]);
  expect(allRequestText(session)).toContain("Closes Falconiere/comemory#255");
  return killed;
}

/** A real checkpoint keeps the killed worker's edit and none of toolu's runtime state. */
async function checkpointKeepsEdit(session: ProbeSession): Promise<void> {
  const p = layout(session.sb.root);
  const checkpoint = await run(
    [process.execPath, join(SCRIPTS, "checkpoint.ts"), "--state-dir", p.state, "--key", KEY],
    { cwd: session.sb.root },
  );
  expect(checkpoint.exitCode, checkpoint.stderr).toBe(0);
  expect(z.array(z.looseObject({})).parse(JSON.parse(checkpoint.stdout))).toMatchObject([
    { key: KEY, changed: true },
  ]);
  const tree = session.sb
    .git("-C", p.worktree, "ls-tree", "-r", "--name-only", `refs/epic-wip/${KEY}`)
    .split("\n");
  expect(tree).toContain("CHANGE.md");
  expect(tree.filter((path) => TOOLU_RUNTIME.test(path))).toEqual([]);
  expect(existsSync(join(p.worktree, ".opencode/tmp"))).toBe(true);
}

/** `--continue` in the same worktree: commit, review state, push, ready, seen by the watcher. */
async function resumeToReady(bin: string, session: ProbeSession) {
  const { sb } = session;
  const p = layout(sb.root);
  const resumed = await runHost(
    bin,
    session,
    [...agentArgs("opencode", { ...ARGS, resume: true }), "PROBE:epic.resume Resume the brief."],
    RUN_TIMEOUT_MS,
    p.worktree,
  );
  expect(resumed.exitCode, resumed.stderr.slice(-4000)).toBe(0);
  const states = toolStates(resumed).map(({ tool, status }) => `${tool}:${status}`);
  expect(states).toEqual(["bash:completed", "bash:completed", "bash:completed", "bash:completed"]);
  const pushed = sb.git("--git-dir", p.remote, "rev-parse", `refs/heads/${ISSUE.branch}`).trim();
  expect(pushed).toBe(sb.git("-C", p.worktree, "rev-parse", "HEAD").trim());
  expect(statusOf(p.status)).toMatchObject({ phase: "ready", pr: 1 });
  const peek = await run(
    [process.execPath, join(SCRIPTS, "epic-watch.ts"), "--state-dir", p.state, "--peek"],
    { cwd: sb.root },
  );
  expect(peek.exitCode, peek.stderr).toBe(0);
  const { events } = z
    .object({ events: z.array(z.looseObject({ type: z.string(), key: z.string().optional() })) })
    .parse(JSON.parse(peek.stdout));
  expect(events.filter((e) => e.key === KEY).map((e) => e.type)).toContain("ready");
  return { sessions: sessionIds(resumed.events.map((e) => Event.parse(e))), states, pushed };
}

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "an OpenCode epic worker survives a mid-turn kill and resumes to a ready report",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const version = await run([host.bin, "--version"], { stdin: "" });
    expect(opencodeVersionProblem(version.stdout)).toBeNull();
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    using session = openSession(cacheRoot, { scripts: workerScripts });
    await epicFixture(session);
    const p = layout(session.sb.root);

    const killed = await killMidTurn(host.bin, session);
    await checkpointKeepsEdit(session);
    const resumed = await resumeToReady(host.bin, session);
    expect(resumed.sessions).toEqual(sessionIds(killed.events));

    const slug = ISSUE.branch.replaceAll("/", "_");
    const telemetry = readFileSync(join(p.worktree, ".opencode/tmp/telemetry", `${slug}.jsonl`));
    expect(telemetry.toString()).toContain('"subagent_type":"toolu-quick-task"');
    const porcelain = session.sb
      .git("-C", p.worktree, "status", "--porcelain", "--untracked-files=all")
      .split("\n")
      .filter(Boolean);
    expect(porcelain.filter((line) => TOOLU_RUNTIME.test(line))).toEqual([]);
    const evidence = { host: host.version, version: version.stdout.trim(), porcelain };
    process.stdout.write(
      `${JSON.stringify({ ...evidence, killedExit: killed.exitCode, ...resumed })}\n`,
    );
  },
  900_000,
);
