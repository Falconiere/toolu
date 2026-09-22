/** Execute one argv spawn with deadline and output limits. */
import type { Subprocess } from "bun";
import { readLimitedStream } from "./runner-read.ts";
import type { BashRunArgs, RawProcessResult } from "./runner-types.ts";

type Spawned = {
  proc: Subprocess<"pipe", "pipe", "pipe">;
  sessionLeader: boolean;
};

function spawnFailure(message: string): RawProcessResult {
  return {
    ok: false,
    code: "spawn",
    message,
    exitCode: null,
    stdout: "",
    stderr: message,
    truncated: false,
  };
}

function killSpawned(spawned: Spawned): void {
  const { proc, sessionLeader } = spawned;
  if (sessionLeader) {
    try {
      process.kill(-proc.pid, "SIGTERM");
      return;
    } catch {
      /* fall through to PID kill */
    }
  }
  try {
    process.kill(proc.pid, "SIGTERM");
  } catch {
    /* already exited */
  }
}

function withSession(argv: string[]): { argv: string[]; sessionLeader: boolean } {
  if (argv[0] === "setsid") {
    return { argv, sessionLeader: true };
  }
  const setsidBin = Bun.which("setsid");
  if (setsidBin !== null) {
    return { argv: [setsidBin, ...argv], sessionLeader: true };
  }
  const python = Bun.which("python3") ?? Bun.which("python");
  if (python !== null) {
    return {
      argv: [
        python,
        "-c",
        "import os, sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])",
        ...argv,
      ],
      sessionLeader: true,
    };
  }
  return { argv, sessionLeader: false };
}

function raceAbort(
  deadlineMs: number,
  signal: AbortSignal | undefined,
): { promise: Promise<"timeout" | "cancelled">; clear: () => void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<"timeout">((resolve) => {
    timer = setTimeout(() => resolve("timeout"), deadlineMs);
  });
  const cancel =
    signal === undefined
      ? null
      : signal.aborted
        ? Promise.resolve("cancelled" as const)
        : new Promise<"cancelled">((resolve) => {
            signal.addEventListener("abort", () => resolve("cancelled"), { once: true });
          });
  const racers: Promise<"timeout" | "cancelled">[] = [deadline];
  if (cancel !== null) {
    racers.push(cancel);
  }
  return {
    promise: Promise.race(racers),
    clear: () => {
      if (timer !== undefined) {
        clearTimeout(timer);
      }
    },
  };
}

async function collectOutputs(
  proc: Subprocess<"pipe", "pipe", "pipe">,
  maxStdoutBytes: number,
): Promise<{
  stdoutOut: Awaited<ReturnType<typeof readLimitedStream>>;
  stderrOut: Awaited<ReturnType<typeof readLimitedStream>>;
  exitCode: number;
}> {
  const [stdoutOut, stderrOut, exitCode] = await Promise.all([
    readLimitedStream(proc.stdout, maxStdoutBytes),
    readLimitedStream(proc.stderr, maxStdoutBytes),
    proc.exited,
  ]);
  return { stdoutOut, stderrOut, exitCode };
}

async function openSpawn(args: BashRunArgs): Promise<Spawned | RawProcessResult> {
  const cmd = args.argv[0];
  if (cmd === undefined || cmd.length === 0) {
    return spawnFailure("empty argv");
  }
  if (!cmd.includes("/") && Bun.which(cmd) === null) {
    return spawnFailure(`command not found: ${cmd}`);
  }
  const wrapped = withSession(args.argv);
  try {
    const proc = Bun.spawn(wrapped.argv, {
      cwd: args.cwd,
      env: { ...process.env, ...args.env },
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    });
    proc.stdin.write(args.stdin);
    const flushed = proc.stdin.flush();
    if (typeof flushed !== "number") {
      await flushed;
    }
    const ended = proc.stdin.end();
    if (typeof ended !== "number") {
      await ended;
    }
    return { proc, sessionLeader: wrapped.sessionLeader };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return spawnFailure(message);
  }
}

async function finishTimedOut(
  spawned: Spawned,
  code: "timeout" | "cancelled",
  maxStdoutBytes: number,
): Promise<RawProcessResult> {
  killSpawned(spawned);
  await spawned.proc.exited;
  const { stdoutOut, stderrOut } = await collectOutputs(spawned.proc, maxStdoutBytes);
  return {
    ok: false,
    code,
    message: code === "cancelled" ? "process cancelled" : "process timed out",
    exitCode: null,
    stdout: stdoutOut.text,
    stderr: stderrOut.text,
    truncated: stdoutOut.truncated || stderrOut.truncated,
  };
}

export async function execBashRun(args: BashRunArgs): Promise<RawProcessResult> {
  const opened = await openSpawn(args);
  if (!("proc" in opened)) {
    return opened;
  }

  const abort = raceAbort(args.deadlineMs, args.signal);
  const raced = await Promise.race([opened.proc.exited.then(() => "done" as const), abort.promise]);

  if (raced !== "done") {
    abort.clear();
    return finishTimedOut(opened, raced, args.maxStdoutBytes);
  }

  abort.clear();
  const { stdoutOut, stderrOut, exitCode } = await collectOutputs(opened.proc, args.maxStdoutBytes);

  if (stdoutOut.truncated) {
    return {
      ok: false,
      code: "truncated",
      message: "stdout exceeded maxStdoutBytes",
      exitCode,
      stdout: stdoutOut.text,
      stderr: stderrOut.text,
      truncated: true,
    };
  }

  return {
    ok: true,
    exitCode,
    stdout: stdoutOut.text,
    stderr: stderrOut.text,
    truncated: false,
  };
}
