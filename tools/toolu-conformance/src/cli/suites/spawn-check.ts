import { existsSync, statSync } from "node:fs";
import type { SuiteOutcome } from "../types.ts";

function probeEnv(): Record<string, string> {
  const out: Record<string, string> = {};
  for (const key of ["PATH", "HOME", "TMPDIR", "LANG"] as const) {
    const value = process.env[key];
    if (value !== undefined) {
      out[key] = value;
    }
  }
  return out;
}

/** Validate OPENCODE_BIN / argv[0] is an existing regular file when absolute. */
export function assertSafeBinary(path: string): SuiteOutcome | null {
  if (path.includes("\0")) {
    return { status: "fail", message: "binary path contains NUL" };
  }
  if (!existsSync(path)) {
    return { status: "fail", message: `binary not found: ${path}` };
  }
  try {
    if (!statSync(path).isFile()) {
      return { status: "fail", message: `binary is not a file: ${path}` };
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { status: "fail", message };
  }
  return null;
}

/** Run argv in cwd; pass when exit 0 (#212). Argv only — no shell. */
export async function runArgvCheck(
  argv: string[],
  options: { cwd?: string; failLabel: string },
): Promise<SuiteOutcome> {
  const binary = argv[0];
  if (binary === undefined || binary.length === 0) {
    return { status: "fail", message: `${options.failLabel}: empty argv` };
  }
  if (binary.includes("/") || binary.startsWith(".")) {
    const bad = assertSafeBinary(binary);
    if (bad) {
      return bad;
    }
  }

  const proc = Bun.spawn(argv, {
    ...(options.cwd !== undefined ? { cwd: options.cwd } : {}),
    stdout: "pipe",
    stderr: "pipe",
    env: probeEnv(),
  });
  const [exitCode, stdout, stderr] = await Promise.all([
    proc.exited,
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ]);
  void stdout;
  if (exitCode === 0) {
    return { status: "pass" };
  }

  const detail = stderr.trim().slice(0, 500);
  return {
    status: "fail",
    message: detail.length > 0 ? detail : `${options.failLabel} exited ${exitCode}`,
  };
}
