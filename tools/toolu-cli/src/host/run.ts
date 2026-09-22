import { spawn } from "node:child_process";
import { constants } from "node:fs";
import { access } from "node:fs/promises";
import { delimiter, join } from "node:path";

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

/**
 * True when the binary resolves on PATH.
 *
 * Resolved by reading PATH directly rather than shelling out: `command` is a
 * shell builtin, so spawning it always fails, and `which` costs a process per
 * probe on a path the CLI walks for every host on every run.
 */
export async function binaryExists(
  bin: string,
  env: NodeJS.ProcessEnv = process.env,
): Promise<boolean> {
  for (const directory of (env.PATH ?? "").split(delimiter)) {
    if (directory.length === 0) continue;
    try {
      await access(join(directory, bin), constants.X_OK);
      return true;
    } catch {
      continue;
    }
  }
  return false;
}
