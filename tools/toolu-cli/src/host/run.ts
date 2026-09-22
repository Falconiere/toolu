import { spawn } from "node:child_process";

interface RunResult {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
}

/** Runs argv directly, never through a shell. */
export function run(
  argv: readonly string[],
  env: NodeJS.ProcessEnv = process.env,
): Promise<RunResult> {
  const [command, ...rest] = argv;
  if (command === undefined) throw new Error("run requires a command");
  return new Promise<RunResult>((settle) => {
    const child = spawn(command, rest, { env, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error: Error) => {
      settle({ code: 127, stdout, stderr: `${stderr}${error.message}` });
    });
    child.on("close", (code: number | null) => {
      settle({ code: code ?? 1, stdout, stderr });
    });
  });
}

/** True when the binary resolves on PATH. */
export async function binaryExists(bin: string, env?: NodeJS.ProcessEnv): Promise<boolean> {
  const result = await run(["command", "-v", bin], env).catch(() => undefined);
  if (result !== undefined && result.code === 0) return true;
  const which = await run(["/usr/bin/which", bin], env).catch(() => undefined);
  return which !== undefined && which.code === 0;
}
