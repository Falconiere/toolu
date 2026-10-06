/**
 * One startup entry as a child process (#342): stdin is the SessionStart
 * payload, stdout and stderr are read to a byte bound, and a deadline or an
 * abort kills the child. Every way the child cannot finish is a reason. Bun
 * runs it with `--no-env-file`, including the native launcher's Bun fallback,
 * so a project `.env` never reaches it (#350).
 */
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const MAX_OUTPUT_BYTES = 512_000;

export type SpawnRequest = {
  bun: string;
  bundle: string;
  /** The #412 generated POSIX command; absent means the Bun launcher. */
  command?: string;
  cwd: string;
  env: Record<string, string>;
  stdin: string;
  deadlineMs: number;
  signal: AbortSignal | undefined;
};

export type SpawnOutcome =
  | { status: "exited"; exitCode: number; stdout: string; stderr: string }
  | { status: "failed"; reason: string };

async function readBoundedOutput(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let bytes = 0;
  let output = "";
  async function readNext(): Promise<string> {
    const { done, value } = await reader.read();
    if (done) return output + decoder.decode();
    bytes += value.byteLength;
    if (bytes > MAX_OUTPUT_BYTES)
      throw new Error(`startup output exceeded ${MAX_OUTPUT_BYTES} bytes`);
    output += decoder.decode(value, { stream: true });
    return readNext();
  }
  try {
    return await readNext();
  } finally {
    reader.releaseLock();
  }
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Why the child was stopped from outside, if it was, and when. */
type Stop = { reason: string | undefined; halted: PromiseWithResolvers<void> };

/**
 * A killed entry's grandchild can keep its output pipes open; past this grace
 * the outcome is reported without waiting for them, so a deadline or an abort
 * always bounds the call.
 */
const DRAIN_GRACE_MS = 1_000;

async function abandoned(stop: Stop): Promise<SpawnOutcome> {
  await stop.halted.promise;
  await Bun.sleep(DRAIN_GRACE_MS);
  return { status: "failed", reason: stop.reason ?? "startup stopped" };
}

async function collect(
  proc: Bun.Subprocess<Blob, "pipe", "pipe">,
  stop: Stop,
): Promise<SpawnOutcome> {
  try {
    const [stdout, stderr, exitCode] = await Promise.all([
      readBoundedOutput(proc.stdout),
      readBoundedOutput(proc.stderr),
      proc.exited,
    ]);
    if (stop.reason !== undefined) return { status: "failed", reason: stop.reason };
    return { status: "exited", exitCode, stdout, stderr };
  } catch (error) {
    proc.kill();
    return { status: "failed", reason: stop.reason ?? message(error) };
  }
}

export async function spawnEntry(request: SpawnRequest): Promise<SpawnOutcome> {
  if (request.signal?.aborted === true) return { status: "failed", reason: "startup cancelled" };
  let proc: Bun.Subprocess<Blob, "pipe", "pipe">;
  let wrapperDir: string | undefined;
  try {
    // The #412 native command executes TOOLU_BUN without flags on fallback. Keep its
    // command intact while making that Bun invocation honor OpenCode's .env boundary.
    let env = request.env;
    if (request.command !== undefined) {
      wrapperDir = mkdtempSync(join(tmpdir(), "toolu-native-bun-"));
      const wrapper = join(wrapperDir, "bun");
      writeFileSync(wrapper, '#!/bin/sh\nexec "$TOOLU_OPENCODE_NATIVE_BUN" --no-env-file "$@"\n', {
        mode: 0o700,
      });
      env = {
        ...request.env,
        TOOLU_BUN: wrapper,
        TOOLU_OPENCODE_NATIVE_BUN: request.bun,
      };
    }
    const argv =
      request.command === undefined
        ? [request.bun, "--no-env-file", request.bundle]
        : ["/bin/sh", "-c", request.command];
    proc = Bun.spawn(argv, {
      cwd: request.cwd,
      env,
      stdin: new Blob([request.stdin]),
      stdout: "pipe",
      stderr: "pipe",
    });
  } catch (error) {
    if (wrapperDir !== undefined) rmSync(wrapperDir, { recursive: true, force: true });
    return { status: "failed", reason: `cannot start: ${message(error)}` };
  }
  const stop: Stop = { reason: undefined, halted: Promise.withResolvers<void>() };
  const halt = (reason: string): void => {
    stop.reason ??= reason;
    proc.kill();
    stop.halted.resolve();
  };
  const timer = setTimeout(
    () => halt(`timed out after ${request.deadlineMs} ms`),
    request.deadlineMs,
  );
  const onAbort = (): void => halt("startup cancelled");
  request.signal?.addEventListener("abort", onAbort, { once: true });
  try {
    return await Promise.race([collect(proc, stop), abandoned(stop)]);
  } finally {
    clearTimeout(timer);
    request.signal?.removeEventListener("abort", onAbort);
    if (wrapperDir !== undefined) rmSync(wrapperDir, { recursive: true, force: true });
  }
}
