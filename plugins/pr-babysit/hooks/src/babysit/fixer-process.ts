/**
 * OpenCode fixers (#357): a detached `opencode run` in the slot worktree, not a
 * herdr pane. Its agent is defined through OPENCODE_CONFIG_CONTENT with remote
 * writes and subagents denied; the host layers agent rules over the user's own,
 * and `--auto` approves asks but keeps explicit denies. The dispatcher records
 * the process group's pid and its leader's start time, so a reused pid is never
 * taken for the fixer or signalled.
 */
import { spawn, spawnSync } from "node:child_process";
import { closeSync, existsSync, openSync, readFileSync } from "node:fs";
import { fail, type Json } from "./common.ts";
import { agentArgs, commandAvailable } from "./fixer-route.ts";

export const FIXER_AGENT = "pr-babysit-fixer";

const DENY = "deny";
const FIXER_AGENT_CONFIG = {
  mode: "primary",
  description: "pr-babysit fixer: edits, tests and commits review fixes in its worktree",
  permission: {
    task: DENY,
    bash: {
      gh: DENY,
      "gh *": DENY,
      "git push": DENY,
      "git push *": DENY,
      "git * push": DENY,
      "git * push *": DENY,
    },
  },
};

function isObject(value: unknown): value is Json {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The caller's OPENCODE_CONFIG_CONTENT with the fixer agent added; every other key is kept. */
export function fixerConfigContent(existing: string | undefined): string {
  let base: Json = {};
  if (existing !== undefined && existing.trim() !== "") {
    let parsed: unknown;
    try {
      parsed = JSON.parse(existing);
    } catch {
      parsed = undefined;
    }
    if (!isObject(parsed))
      fail("config_invalid", "OPENCODE_CONFIG_CONTENT is not a JSON object; fix or unset it");
    if (parsed.agent !== undefined && !isObject(parsed.agent))
      fail("config_invalid", "OPENCODE_CONFIG_CONTENT agent is not an object");
    base = parsed;
  }
  const agents = isObject(base.agent) ? base.agent : {};
  return JSON.stringify({ ...base, agent: { ...agents, [FIXER_AGENT]: FIXER_AGENT_CONFIG } });
}

export type FixerRun = {
  model: string | null;
  effort: string | null;
  unattended: boolean;
  worktree: string;
  prompt: string;
};

/** `opencode run` arguments for one fixer group; model and effort are shell-safe checked. */
export function opencodeFixerArgs(run: FixerRun): string[] {
  return [
    "run",
    "--format",
    "json",
    "--dir",
    run.worktree,
    "--agent",
    FIXER_AGENT,
    ...agentArgs("opencode", FIXER_AGENT, run.model, run.effort, run.unattended),
    run.prompt,
  ];
}

/** The leader's start time as `ps` prints it, or "" when no such process exists. */
export function processStart(pid: number): string {
  const res = spawnSync("ps", ["-o", "lstart=", "-p", String(pid)], { encoding: "utf8" });
  return res.status === 0 ? res.stdout.trim() : "";
}

/** Live (non-zombie) members of process group `pgid`. */
function groupMembers(pgid: number): number[] {
  const res = spawnSync("ps", ["-A", "-o", "pid=,pgid=,stat="], { encoding: "utf8" });
  if (res.status !== 0) fail("process_error", `ps failed: ${(res.stderr ?? "").trim()}`);
  return res.stdout.split("\n").flatMap((line) => {
    const [pid, group, stat] = line.trim().split(/\s+/);
    return group === String(pgid) && stat !== undefined && !stat.startsWith("Z")
      ? [Number(pid)]
      : [];
  });
}

/** Is the fixer group started as `pid` at `start` still running? A reused pid is not. */
export function groupAlive(pid: number, start: string): boolean {
  if (!Number.isInteger(pid) || pid <= 1) return false;
  const leader = processStart(pid);
  if (leader !== "" && leader !== start) return false;
  return groupMembers(pid).length > 0;
}

function signalGroup(pid: number, signal: NodeJS.Signals): void {
  try {
    process.kill(-pid, signal);
  } catch (error) {
    // ESRCH: the group exited between the liveness check and the signal.
    if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
  }
}

function waitGone(pid: number, start: string, seconds: number): boolean {
  for (let i = 0; i < seconds * 10; i += 1) {
    if (!groupAlive(pid, start)) return true;
    Bun.sleepSync(100);
  }
  return !groupAlive(pid, start);
}

/** End the fixer group: TERM, up to 10 s, then KILL. True when nothing of it is left. */
export function stopGroup(pid: number, start: string): boolean {
  if (!groupAlive(pid, start)) return true;
  signalGroup(pid, "SIGTERM");
  if (waitGone(pid, start, 10)) return true;
  signalGroup(pid, "SIGKILL");
  return waitGone(pid, start, 5);
}

export type Spawned = { pid: number; pidStart: string } | { error: string };

/** Start `opencode run` detached in `worktree`, output to `log`; never waits for it. */
export function spawnFixer(run: FixerRun, log: string, env: NodeJS.ProcessEnv): Spawned {
  if (!commandAvailable("opencode")) return { error: "opencode is not on PATH" };
  const args = opencodeFixerArgs(run);
  const content = fixerConfigContent(env.OPENCODE_CONFIG_CONTENT);
  const fd = openSync(log, "a");
  try {
    const child = spawn("opencode", args, {
      cwd: run.worktree,
      // OpenCode takes its project from PWD, not from the process cwd.
      env: { ...env, PWD: run.worktree, OPENCODE_CONFIG_CONTENT: content },
      detached: true,
      stdio: ["ignore", fd, fd],
    });
    // A start failure leaves `pid` unset and is reported below; without a listener it would throw.
    child.on("error", () => undefined);
    child.unref();
    if (child.pid === undefined) return { error: "opencode did not start" };
    return { pid: child.pid, pidStart: processStart(child.pid) };
  } catch (error) {
    return { error: `opencode did not start: ${(error as Error).message}` };
  } finally {
    closeSync(fd);
  }
}

/** The last `lines` lines of the fixer's log, or "" when it has none. */
export function logTail(log: string, lines = 40): string {
  if (!existsSync(log)) return "";
  return readFileSync(log, "utf8").split("\n").slice(-lines).join("\n");
}
