/**
 * One startup entry as a child process (#342): stdin is the SessionStart
 * payload, stdout and stderr are read to a byte bound, and a deadline or an
 * abort kills the child. Every way the child cannot finish is a reason.
 */
const MAX_OUTPUT_BYTES = 512_000;

export type SpawnRequest = {
  bun: string;
  bundle: string;
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

/** Why the child was stopped from outside, if it was. */
type Stop = { reason: string | undefined };

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
  try {
    proc = Bun.spawn([request.bun, request.bundle], {
      cwd: request.cwd,
      env: request.env,
      stdin: new Blob([request.stdin]),
      stdout: "pipe",
      stderr: "pipe",
    });
  } catch (error) {
    return { status: "failed", reason: `cannot start: ${message(error)}` };
  }
  const stop: Stop = { reason: undefined };
  const halt = (reason: string): void => {
    stop.reason ??= reason;
    proc.kill();
  };
  const timer = setTimeout(
    () => halt(`timed out after ${request.deadlineMs} ms`),
    request.deadlineMs,
  );
  const onAbort = (): void => halt("startup cancelled");
  request.signal?.addEventListener("abort", onAbort, { once: true });
  try {
    return await collect(proc, stop);
  } finally {
    clearTimeout(timer);
    request.signal?.removeEventListener("abort", onAbort);
  }
}
