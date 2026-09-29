/**
 * How a bundle's result is compared with a bash capture (#260): the decision
 * JSON parsed (the native encoder prints compact JSON where `jq -n`
 * pretty-printed), the exit code, and stderr without `toolu-config:` lines (the
 * dispatcher prints each config warning once; each bash module repeated it).
 */
import type { Captured } from "./pre-tool-modules-a-cases.ts";

function json(stdout: string): unknown {
  return stdout.trim() === "" ? null : JSON.parse(stdout);
}

function stderrOf(stderr: string): string {
  return stderr
    .split("\n")
    .filter((line) => !line.startsWith("toolu-config: "))
    .join("\n");
}

export function comparable(result: Captured): {
  decision: unknown;
  stderr: string;
  exitCode: number;
} {
  return {
    decision: json(result.stdout),
    stderr: stderrOf(result.stderr),
    exitCode: result.exitCode,
  };
}
