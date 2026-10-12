/**
 * Live epic worker (#356, AC-3 and AC-9; #435 AC-23), reported apart from the
 * hermetic suites. `TOOLU_LIVE_OPENCODE=1` runs the pinned host as an epic
 * worker in a linked worktree of an isolated fixture repo, with the flags,
 * start prompt, brief and `info/exclude` lines `toolu epic launch --dry-run
 * --json` prints, and toolu loaded from the global config. The worker reports
 * through `toolu epic report`, loads delivery, delegates through `task`, edits
 * and is killed mid-turn. The edit survives; `--session` with the id `session
 * list` captured (the resume `toolu epic launch` plans) resumes it through
 * commit, review, push and a ready report. Both `toolu` commands resolve
 * through the conformance seam's Rust-only entries.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { rustOnlyArgv } from "@toolu/conformance/harness/entry-command";
import { childEnv, run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
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

const GRAPH_FILE = join(ROOT, "fixtures/epic/epic248-graph.json");
const GraphIssue = z.looseObject({ ref: z.string(), key: z.string(), branch: z.string() });
const GRAPH = z.looseObject({ issues: z.array(GraphIssue) }).parse(
  JSON.parse(readFileSync(GRAPH_FILE, "utf8")),
);
const ISSUE = ((): z.infer<typeof GraphIssue> => {
  const found = GRAPH.issues.find((issue) => issue.ref === "Falconiere/comemory#255");
  if (found === undefined) throw new Error("the epic248 graph has no comemory#255");
  return found;
})();
const KEY = "comemory-255";
const MODEL = "probe/scripted";
const TOOLU_RUNTIME = /\.opencode\/(toolu\/state|tmp)\//;
/** The launch plan `toolu epic launch --dry-run --json` prints. */
const Plan = z.looseObject({
  agent_args: z.array(z.string()),
  brief: z.string(),
  brief_path: z.string(),
  prompt: z.string(),
  exclude: z.array(z.string()),
  resume: z.boolean(),
});

/** One argv word as a POSIX shell word. */
const shellWord = (word: string) => `'${word.replaceAll("'", "'\\''")}'`;

/** `toolu epic report` as a shell command, through the seam. */
const report = () => rustOnlyArgv("epic-orchestrator", "report").map(shellWord).join(" ");

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
      bash(`${report()} execution --status-file "${p.status}"`),
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
      bash(`${report()} ready --pr 1 --status-file "${p.status}"`),
    ],
  };
}

function write(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

/**
 * `toolu epic launch --dry-run --json` for the issue on OpenCode: the plan a
 * real launch would follow, with nothing written.
 */
async function plan(session: ProbeSession): Promise<z.infer<typeof Plan>> {
  const p = layout(session.sb.root);
  const graph = join(session.sb.root, "graph.json");
  const doc = { ...JSON.parse(readFileSync(GRAPH_FILE, "utf8")), state_dir: p.state };
  write(graph, JSON.stringify(doc));
  const [bin = "toolu", ...verb] = rustOnlyArgv("epic-orchestrator", "launch");
  const flags = ["--graph", graph, "--issue", ISSUE.ref, "--dry-run", "--kind", "opencode"];
  const planned = await run([bin, "--json", ...verb, ...flags, "--model", MODEL], {
    cwd: session.sb.root,
    env: session.env,
    stdin: "",
  });
  expect(planned.exitCode, planned.stderr).toBe(0);
  return Plan.parse(JSON.parse(planned.stdout));
}

/** A repo with a bare remote, a committed selection, the worker's linked worktree and epic state. */
async function epicFixture(session: ProbeSession): Promise<z.infer<typeof Plan>> {
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
  const fresh = await plan(session);
  expect(fresh.resume).toBe(false);
  expect(fresh.agent_args).toContain("--auto");
  expect(fresh.brief_path).toBe(p.brief);
  write(p.brief, fresh.brief);
  const common = sb.git("-C", p.worktree, "rev-parse", "--path-format=absolute", "--git-common-dir");
  const exclude = join(common.trim(), "info", "exclude");
  write(exclude, `${existsSync(exclude) ? readFileSync(exclude, "utf8") : ""}${fresh.exclude.join("\n")}\n`);
  return fresh;
}

/** Record the running worker with its captured session, as a launch does. */
function recordWorker(session: ProbeSession, sessionId: string): void {
  const p = layout(session.sb.root);
  const record = { ref: ISSUE.ref, kind: "opencode", stage: "running", agent: KEY, launches: 1 };
  write(
    join(p.state, "issues", `${KEY}.json`),
    JSON.stringify({ ...record, worktree: p.worktree, branch: ISSUE.branch, session_id: sessionId }),
  );
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

/** Session ids `session list` attributes to `worktree`: the list `toolu epic launch` reads to capture the session. */
async function listedSessions(bin: string, session: ProbeSession, worktree: string) {
  const listed = await run([bin, "session", "list", "--format", "json"], {
    cwd: worktree,
    env: { ...session.env, PWD: worktree },
    stdin: "",
  });
  expect(listed.exitCode, listed.stderr).toBe(0);
  return z
    .array(z.looseObject({ id: z.string(), directory: z.string(), created: z.number() }))
    .parse(JSON.parse(listed.stdout))
    .filter((entry) => entry.directory === worktree)
    .map((entry) => entry.id);
}

/** The first run: start prompt, delivery, one `task`, an edit, then a kill mid-turn. */
async function killMidTurn(bin: string, session: ProbeSession, fresh: z.infer<typeof Plan>) {
  const p = layout(session.sb.root);
  const prompt = `${fresh.prompt} PROBE:epic.worker`;
  const killed = await runUntilKilled(bin, session, [...fresh.agent_args, prompt], p.worktree);
  expect(killed.atKillPoint, killed.stderr.slice(-4000)).toBe(true);
  expect(killed.exitCode).not.toBe(0);
  expect(statusOf(p.status).phase).toBe("execution");
  const states = toolStates({ events: killed.events });
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

/** The killed worker's edit is still in the worktree, and none of toolu's runtime state shows. */
function editSurvives(session: ProbeSession): void {
  const p = layout(session.sb.root);
  expect(readFileSync(join(p.worktree, "CHANGE.md"), "utf8")).toBe("Bounded.\n");
  const porcelain = session.sb.git("-C", p.worktree, "status", "--porcelain", "--untracked-files=all");
  expect(porcelain).toContain("CHANGE.md");
  expect(porcelain.split("\n").filter((line) => TOOLU_RUNTIME.test(line))).toEqual([]);
  expect(existsSync(join(p.worktree, ".opencode/tmp"))).toBe(true);
}

/** `--session <captured>` in the same worktree, as the launch plans it: commit, review state, push, ready. */
async function resumeToReady(bin: string, session: ProbeSession, sessionId: string) {
  const { sb } = session;
  const p = layout(sb.root);
  recordWorker(session, sessionId);
  const resume = await plan(session);
  expect(resume.resume).toBe(true);
  expect(resume.agent_args).toContain(sessionId);
  expect(resume.agent_args).toContain("--auto");
  const args = [...resume.agent_args, "PROBE:epic.resume Resume the brief."];
  const resumed = await runHost(bin, session, args, RUN_TIMEOUT_MS, p.worktree);
  expect(resumed.exitCode, resumed.stderr.slice(-4000)).toBe(0);
  const states = toolStates(resumed).map(({ tool, status }) => `${tool}:${status}`);
  expect(states).toEqual(["bash:completed", "bash:completed", "bash:completed", "bash:completed"]);
  const pushed = sb.git("--git-dir", p.remote, "rev-parse", `refs/heads/${ISSUE.branch}`).trim();
  expect(pushed).toBe(sb.git("-C", p.worktree, "rev-parse", "HEAD").trim());
  expect(statusOf(p.status)).toMatchObject({ phase: "ready", pr: 1 });
  return { sessions: sessionIds(resumed.events.map((e) => Event.parse(e))), states, pushed };
}

test.skipIf(process.env.TOOLU_LIVE_OPENCODE !== "1")(
  "an OpenCode epic worker survives a mid-turn kill and resumes to a ready report",
  async () => {
    const pin = readJson(contractPaths().pin, PinSchema);
    const host = await resolveHostBinary(pin);
    const version = await run([host.bin, "--version"], { stdin: "" });
    expect(version.stdout).toMatch(/\b1\.\d+\.\d+/);
    const cacheRoot = join(hostCacheDir(pin), "run-cache");
    mkdirSync(cacheRoot, { recursive: true });
    using session = openSession(cacheRoot, { scripts: workerScripts });
    const fresh = await epicFixture(session);
    const p = layout(session.sb.root);

    const killed = await killMidTurn(host.bin, session, fresh);
    const listed = await listedSessions(host.bin, session, p.worktree);
    expect(listed).toEqual(sessionIds(killed.events));
    const [captured] = listed;
    if (captured === undefined) throw new Error("session list captured no worker session");
    editSurvives(session);
    const resumed = await resumeToReady(host.bin, session, captured);
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
