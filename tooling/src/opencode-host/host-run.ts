/**
 * Drive the pinned OpenCode binary inside a probe session (#335).
 * `opencode run` reads a non-TTY stdin and blocks waiting for more input, so
 * every run gets an empty, closed stdin. Each run has a hard timeout; a hung
 * host is killed with its process group and reported; it never becomes a verdict.
 */
import { existsSync, readFileSync } from "node:fs";
import { z } from "zod";
import { childEnv, run } from "@toolu/conformance/harness/spawn";
import type { ProbeSession } from "./session.ts";
import { ContractError } from "./schema.ts";

/**
 * A hang guard, not a budget. A fresh isolated profile makes the host install
 * its SDK into the profile's config directories before any plugin loads. On a
 * busy host that alone took 60-130 s per run (#342), so 120 s cut off runs that
 * were still working.
 */
export const RUN_TIMEOUT_MS = 300_000;
const SERVE_READY_TIMEOUT_MS = 60_000;

type HostRun = { exitCode: number; events: Array<Record<string, unknown>>; stderr: string };

const JsonRecord = z.record(z.string(), z.unknown());

function parseLine(line: string): unknown {
  try {
    return JSON.parse(line);
  } catch {
    return null;
  }
}

/** The JSON event lines of `opencode run --format json`; other output is skipped. */
function jsonLines(text: string): Array<Record<string, unknown>> {
  return text.split("\n").flatMap((line) => {
    const parsed = JsonRecord.safeParse(parseLine(line));
    return parsed.success ? [parsed.data] : [];
  });
}

/** `opencode run --format json <args>` in the session project. */
export async function runHost(
  bin: string,
  session: ProbeSession,
  args: string[],
): Promise<HostRun> {
  const res = await run([bin, "run", "--format", "json", ...args], {
    cwd: session.sb.project,
    env: session.env,
    stdin: "",
    timeoutMs: RUN_TIMEOUT_MS,
  });
  if (res.timedOut)
    throw new ContractError(`opencode run ${args.join(" ")} timed out after ${RUN_TIMEOUT_MS} ms`);
  return { exitCode: res.exitCode, events: jsonLines(res.stdout), stderr: res.stderr };
}

/** Parse the stdout of `opencode debug <args>` into JSON. */
export async function debugJson(
  bin: string,
  session: ProbeSession,
  args: string[],
): Promise<unknown> {
  const res = await run([bin, "debug", ...args], {
    cwd: session.sb.project,
    env: session.env,
    stdin: "",
    timeoutMs: RUN_TIMEOUT_MS,
  });
  if (res.timedOut || res.exitCode !== 0) {
    throw new ContractError(
      `opencode debug ${args.join(" ")} failed (exit ${res.exitCode}): ${res.stderr.trim()}`,
    );
  }
  return JSON.parse(res.stdout);
}

/** The tool parts of `opencode run --format json` events, by tool name. */
export function toolStates(
  hostRun: HostRun,
): Array<{ tool: string; status: string; error: string | null }> {
  const ToolUse = z.looseObject({
    type: z.literal("tool_use"),
    part: z.looseObject({
      tool: z.string(),
      state: z.looseObject({ status: z.string(), error: z.string().nullish() }),
    }),
  });
  return hostRun.events.flatMap((event) => {
    const parsed = ToolUse.safeParse(event);
    if (!parsed.success) return [];
    const { tool, state } = parsed.data.part;
    return [{ tool, status: state.status, error: state.error ?? null }];
  });
}

/** Poll the server's stdout file for its listening address until `deadline`. */
async function serverAddress(
  stdoutPath: string,
  proc: Bun.Subprocess,
  deadline: number,
): Promise<string> {
  const out = existsSync(stdoutPath) ? readFileSync(stdoutPath, "utf8") : "";
  const match = out.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
  if (match?.[1] !== undefined) return match[1];
  if (proc.exitCode !== null)
    throw new ContractError(`opencode serve exited ${proc.exitCode}: ${out.trim()}`);
  if (Date.now() >= deadline)
    throw new ContractError(`opencode serve did not start within ${SERVE_READY_TIMEOUT_MS} ms`);
  await Bun.sleep(100);
  return serverAddress(stdoutPath, proc, deadline);
}

/** Run `fn` against `opencode serve` in the session project, then kill the server's process group. */
export async function withServe<T>(
  bin: string,
  session: ProbeSession,
  fn: (url: string) => Promise<T>,
): Promise<T> {
  const stdoutPath = session.outside("serve.stdout");
  const proc = Bun.spawn([bin, "serve", "--port", "0"], {
    cwd: session.sb.project,
    env: childEnv(session.env),
    stdin: "ignore",
    stdout: Bun.file(stdoutPath),
    stderr: Bun.file(session.outside("serve.stderr")),
    detached: true,
  });
  try {
    return await fn(await serverAddress(stdoutPath, proc, Date.now() + SERVE_READY_TIMEOUT_MS));
  } finally {
    killGroup(proc);
  }
}

function killGroup(proc: Bun.Subprocess): void {
  try {
    process.kill(-proc.pid, "SIGKILL");
  } catch (err: unknown) {
    // ESRCH: the group already exited. Anything else is a real failure to clean up.
    if (!(err instanceof Error && "code" in err && err.code === "ESRCH")) throw err;
  }
}
