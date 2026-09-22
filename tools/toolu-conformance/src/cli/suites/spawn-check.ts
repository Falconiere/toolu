import type { SuiteOutcome } from "../types.ts";

/** Run argv in cwd; pass when exit 0 (#212). */
export async function runArgvCheck(
  argv: string[],
  options: { cwd?: string; failLabel: string },
): Promise<SuiteOutcome> {
  const proc = Bun.spawn(argv, {
    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env },
  });
  const [exitCode, stderr] = await Promise.all([proc.exited, new Response(proc.stderr).text()]);

  if (exitCode === 0) {
    return { status: "pass" };
  }

  const detail = stderr.trim().slice(0, 500);
  return {
    status: "fail",
    message: detail.length > 0 ? detail : `${options.failLabel} exited ${exitCode}`,
  };
}
