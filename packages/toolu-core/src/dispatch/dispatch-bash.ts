/**
 * Running a not-yet-ported bash module (#258). `dispatch.sh` runs each module as
 * `result=$(bash "$script" <<<"$input" 2>"$err_file")` with no deadline, so this
 * does the same with one synchronous spawn: the runner in `@toolu/core/runner`
 * would add a `setsid`/`python3` process per module on macOS.
 */
import { constants } from "node:os";
import type { HostEnv } from "../host/host-name.ts";
import { substituted } from "./dispatch-output.ts";

export type ModuleResult = { stdout: string; stderr: string; exitCode: number };

/** Child environment: the host's variables plus `extra`, unset values dropped. */
export function childEnv(env: HostEnv, extra: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value;
  }
  return { ...out, ...extra };
}

/** The status a shell reports: the exit code, or 128 + the signal number when signalled. */
function shellStatus(proc: { exitCode: number; signalCode?: string | undefined }): number {
  if (proc.signalCode === undefined) return proc.exitCode;
  const signals: Record<string, number | undefined> = constants.signals;
  return 128 + (signals[proc.signalCode] ?? 0);
}

/**
 * `bash <script>` fed `stdin`: stdout as `$(...)` keeps it (trailing newlines
 * stripped), stderr as written, and the exit status bash would see. A spawn
 * failure is 127, the status bash gives a command it cannot run.
 */
export function runBash(script: string, stdin: string, env: Record<string, string>): ModuleResult {
  try {
    const proc = Bun.spawnSync(["bash", script], {
      stdin: new TextEncoder().encode(stdin),
      stdout: "pipe",
      stderr: "pipe",
      env,
    });
    return {
      stdout: substituted(proc.stdout.toString()),
      stderr: proc.stderr.toString(),
      exitCode: shellStatus(proc),
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { stdout: "", stderr: `${message}\n`, exitCode: 127 };
  }
}

/** The raw text a module receives: `$(cat)` then `<<<"$input"`. */
export function moduleStdin(input: string): string {
  return `${input}\n`;
}
