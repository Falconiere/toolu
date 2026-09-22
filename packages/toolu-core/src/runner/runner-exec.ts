/** Execute one argv spawn with deadline and output limits. */
import type { Subprocess } from "bun";
import { readLimitedStream } from "./runner-read.ts";
import type { BashRunArgs, RawProcessResult } from "./runner-types.ts";

function killProcessGroup(pid: number): void {
  // Negative PID = process group (child is group leader via setsid when available).
  try {
    process.kill(-pid, "SIGTERM");
  } catch {
    try {
      process.kill(pid, "SIGTERM");
    } catch {
      /* already exited */
    }
  }
}

function spawnArgv(argv: string[]): string[] {
  // Own session/group so timeout can SIGTERM the whole tree, not only the root PID.
  if (argv[0] === "setsid") {
    return argv;
  }
  const setsidBin = Bun.which("setsid");
  if (setsidBin !== null) {
    return [setsidBin, ...argv];
  }
  // macOS has setsid(2) but no setsid(1); start a new session via python3.
  const python = Bun.which("python3") ?? Bun.which("python");
  if (python !== null) {
    return [
      python,
      "-c",
      "import os, sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])",
      ...argv,
    ];
  }
  return argv;
}

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

export async function execBashRun(args: BashRunArgs): Promise<RawProcessResult> {
  const cmd = args.argv[0];
  if (cmd === undefined || cmd.length === 0) {
    return spawnFailure("empty argv");
  }
  // Resolve before session wrap so a missing binary is spawn failure, not wrapper exit 1.
  if (!cmd.includes("/") && Bun.which(cmd) === null) {
    return spawnFailure(`command not found: ${cmd}`);
  }

  let proc;
  try {
    proc = Bun.spawn(spawnArgv(args.argv), {
      cwd: args.cwd,
      env: { ...process.env, ...args.env },
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    });
    void proc.stdin.write(args.stdin);
    void proc.stdin.end();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return spawnFailure(message);
  }

  const abort = raceAbort(args.deadlineMs, args.signal);
  const raced = await Promise.race([proc.exited.then(() => "done" as const), abort.promise]);

  if (raced !== "done") {
    killProcessGroup(proc.pid);
    await proc.exited.catch(() => undefined);
    abort.clear();
    const { stdoutOut, stderrOut } = await collectOutputs(proc, args.maxStdoutBytes);
    return {
      ok: false,
      code: raced,
      message: raced === "cancelled" ? "process cancelled" : "process timed out",
      exitCode: null,
      stdout: stdoutOut.text,
      stderr: stderrOut.text,
      truncated: stdoutOut.truncated || stderrOut.truncated,
    };
  }

  abort.clear();
  const { stdoutOut, stderrOut, exitCode } = await collectOutputs(proc, args.maxStdoutBytes);

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
