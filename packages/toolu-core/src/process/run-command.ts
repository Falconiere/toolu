import { constants } from "node:os";
import { guardParentLifecycle } from "./parent-guard.ts";
import { processGroupAlive, terminateProcessGroup } from "./process-group.ts";

export const DEFAULT_TIMEOUT_MS = 30_000;
export const DEFAULT_MAX_OUTPUT_BYTES = 1_048_576;

const MAX_TIMER_MS = 2_147_483_647;
/** At most four process-table probes per second while redirected descendants remain. */
const GROUP_POLL_MS = 250;
const FINAL_DRAIN_MS = 250;
const CANCELLED_EXIT_CODE = 130;

export type CommandEnvironment = Readonly<Record<string, string | undefined>>;

export interface RunCommandOptions {
  readonly cwd?: string;
  readonly env?: CommandEnvironment;
  readonly stdin?: string | Uint8Array;
  readonly timeoutMs?: number;
  /** Aggregate bytes retained across stdout and stderr. Both streams are always drained. */
  readonly maxOutputBytes?: number;
  readonly signal?: AbortSignal;
  /** Called immediately after spawn with the new process-group id, which is also the child pid. */
  readonly onSpawn?: (pid: number) => void | Promise<void>;
}

export interface RunCommandResult {
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
  readonly durationMs: number;
  readonly timedOut: boolean;
  readonly cancelled: boolean;
  readonly truncated: boolean;
}

type OutputBudget = { remaining: number; truncated: boolean };
type StopReason = "timeout" | "cancelled";

interface Drain {
  readonly done: Promise<string>;
  readonly cancel: () => Promise<void>;
}

interface StopControl {
  readonly stopped: Promise<StopReason>;
  readonly reason: () => StopReason | undefined;
  readonly release: () => void;
}

function errno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

function validateOptions(
  argv: readonly string[],
  options: RunCommandOptions,
): {
  timeoutMs: number;
  maxOutputBytes: number;
} {
  if (argv.length === 0 || argv[0] === undefined || argv[0] === "") {
    throw new Error("argv must not be empty");
  }
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
  if (!Number.isFinite(timeoutMs) || timeoutMs <= 0 || timeoutMs > MAX_TIMER_MS) {
    throw new Error("timeoutMs must be a positive finite number within the timer range");
  }
  const maxOutputBytes = options.maxOutputBytes ?? DEFAULT_MAX_OUTPUT_BYTES;
  if (!Number.isSafeInteger(maxOutputBytes) || maxOutputBytes < 0) {
    throw new Error("maxOutputBytes must be a non-negative safe integer");
  }
  return { timeoutMs, maxOutputBytes };
}

function drain(stream: ReadableStream<Uint8Array>, budget: OutputBudget): Drain {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  const readNext = async (): Promise<void> => {
    const next = await reader.read();
    if (next.done) return;
    const take = Math.min(next.value.byteLength, budget.remaining);
    if (take > 0) chunks.push(next.value.slice(0, take));
    budget.remaining -= take;
    if (take < next.value.byteLength) budget.truncated = true;
    return readNext();
  };
  const done = (async (): Promise<string> => {
    try {
      await readNext();
    } finally {
      reader.releaseLock();
    }
    const length = chunks.reduce((total, chunk) => total + chunk.byteLength, 0);
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.byteLength;
    }
    return new TextDecoder().decode(bytes);
  })();
  return {
    done,
    async cancel(): Promise<void> {
      try {
        await reader.cancel();
      } catch {
        // A completed reader has already released its lock.
      }
    },
  };
}

async function feedStdin(
  proc: Bun.Subprocess<"pipe", "pipe", "pipe">,
  input: string | Uint8Array,
): Promise<void> {
  try {
    await proc.stdin.write(input);
    await proc.stdin.end();
  } catch (error) {
    if (!errno(error, "EPIPE")) throw error;
  }
}

async function waitForGroupExit(
  processGroupId: number,
  stopped: Promise<StopReason>,
): Promise<void> {
  if (!processGroupAlive(processGroupId)) return;
  const outcome = await Promise.race([
    Bun.sleep(GROUP_POLL_MS).then(() => "poll" as const),
    stopped,
  ]);
  if (outcome === "poll") return waitForGroupExit(processGroupId, stopped);
}

async function settleDrains(stdout: Drain, stderr: Drain): Promise<void> {
  const drained = Promise.all([stdout.done, stderr.done]).then(() => true);
  const finished = await Promise.race([drained, Bun.sleep(FINAL_DRAIN_MS).then(() => false)]);
  if (finished) return;
  await Promise.all([stdout.cancel(), stderr.cancel()]);
  await Promise.all([stdout.done, stderr.done]);
}

function exitCode(proc: Bun.Subprocess, observed: number | undefined): number {
  if (observed !== undefined) return observed;
  if (proc.exitCode !== null) return proc.exitCode;
  if (typeof proc.signalCode === "number") return 128 + proc.signalCode;
  const signalNumber = proc.signalCode === null ? undefined : constants.signals[proc.signalCode];
  return 128 + (signalNumber ?? 0);
}

function stopControl(timeoutMs: number, signal: AbortSignal | undefined): StopControl {
  const deferred = Promise.withResolvers<StopReason>();
  let current: StopReason | undefined;
  const stop = (reason: StopReason): void => {
    if (current !== undefined) return;
    current = reason;
    deferred.resolve(reason);
  };
  const timeout = setTimeout(() => stop("timeout"), timeoutMs);
  const onAbort = (): void => stop("cancelled");
  signal?.addEventListener("abort", onAbort, { once: true });
  return {
    stopped: deferred.promise,
    reason: () => current,
    release(): void {
      clearTimeout(timeout);
      signal?.removeEventListener("abort", onAbort);
    },
  };
}

function registerSpawn(callback: RunCommandOptions["onSpawn"], pid: number): Promise<void> {
  try {
    return Promise.resolve(callback?.(pid));
  } catch (error) {
    return Promise.reject(error);
  }
}

async function normalCompletion(
  proc: Bun.Subprocess<"pipe", "pipe", "pipe">,
  prerequisites: readonly Promise<unknown>[],
  stdout: Drain,
  stderr: Drain,
  stop: StopControl,
): Promise<void> {
  await Promise.all(prerequisites);
  await waitForGroupExit(proc.pid, stop.stopped);
  if (stop.reason() === undefined) await Promise.all([stdout.done, stderr.done]);
}

async function stopCommand(
  proc: Bun.Subprocess<"pipe", "pipe", "pipe">,
  exited: Promise<number>,
  stdout: Drain,
  stderr: Drain,
): Promise<void> {
  await terminateProcessGroup(proc.pid);
  await Promise.allSettled([exited]);
  await settleDrains(stdout, stderr);
}

function commandResult(
  proc: Bun.Subprocess,
  observedExitCode: number | undefined,
  started: number,
  stop: StopControl,
  budget: OutputBudget,
  stdout: string,
  stderr: string,
): RunCommandResult {
  return {
    stdout,
    stderr,
    exitCode: exitCode(proc, observedExitCode),
    durationMs: performance.now() - started,
    timedOut: stop.reason() === "timeout",
    cancelled: stop.reason() === "cancelled",
    truncated: budget.truncated,
  };
}

async function monitorCommand(
  proc: Bun.Subprocess<"pipe", "pipe", "pipe">,
  options: RunCommandOptions,
  started: number,
  timeoutMs: number,
  maxOutputBytes: number,
): Promise<RunCommandResult> {
  const budget: OutputBudget = { remaining: maxOutputBytes, truncated: false };
  const stdout = drain(proc.stdout, budget);
  const stderr = drain(proc.stderr, budget);
  const stop = stopControl(timeoutMs, options.signal);
  let observedExitCode: number | undefined;
  const exited = proc.exited.then((code) => (observedExitCode = code));
  const registered = registerSpawn(options.onSpawn, proc.pid);
  const fed = registered.then(() => feedStdin(proc, options.stdin ?? ""));
  const normal = normalCompletion(proc, [fed, exited], stdout, stderr, stop);
  const outcome = normal.then(
    () => ({ kind: "complete" as const }),
    (error: unknown) => ({ kind: "error" as const, error }),
  );
  try {
    const first = await Promise.race([
      outcome,
      stop.stopped.then(() => ({ kind: "stopped" as const })),
    ]);
    if (first.kind !== "complete") await stopCommand(proc, exited, stdout, stderr);
    if (first.kind === "error") throw first.error;
    return commandResult(
      proc,
      observedExitCode,
      started,
      stop,
      budget,
      await stdout.done,
      await stderr.done,
    );
  } finally {
    stop.release();
  }
}

async function execute(
  argv: readonly string[],
  options: RunCommandOptions,
  timeoutMs: number,
  maxOutputBytes: number,
): Promise<RunCommandResult> {
  const started = performance.now();
  if (options.signal?.aborted === true) {
    return {
      stdout: "",
      stderr: "",
      exitCode: CANCELLED_EXIT_CODE,
      durationMs: performance.now() - started,
      timedOut: false,
      cancelled: true,
      truncated: false,
    };
  }

  const spawnOptions: Bun.SpawnOptions.OptionsObject<"pipe", "pipe", "pipe"> = {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
    detached: true,
  };
  if (options.cwd !== undefined) spawnOptions.cwd = options.cwd;
  if (options.env !== undefined) spawnOptions.env = options.env;
  const proc = Bun.spawn([...argv], spawnOptions);
  const releaseParentGuard = guardParentLifecycle(proc.pid);
  try {
    return await monitorCommand(proc, options, started, timeoutMs, maxOutputBytes);
  } finally {
    releaseParentGuard();
  }
}

/** Run one command in an owned process group and resolve only after its live descendants exit. */
export function runCommand(
  argv: readonly string[],
  options: RunCommandOptions = {},
): Promise<RunCommandResult> {
  const { timeoutMs, maxOutputBytes } = validateOptions(argv, options);
  return execute(argv, options, timeoutMs, maxOutputBytes);
}
