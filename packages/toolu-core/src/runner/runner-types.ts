/** Shared runner result types. */

export type RawProcessFailureCode = "timeout" | "spawn" | "truncated" | "cancelled" | "ok";

export type RawProcessResult =
  | {
      ok: true;
      exitCode: number;
      stdout: string;
      stderr: string;
      truncated: boolean;
    }
  | {
      ok: false;
      code: Exclude<RawProcessFailureCode, "ok">;
      message: string;
      exitCode: number | null;
      stdout: string;
      stderr: string;
      truncated: boolean;
    };

export type BashRunArgs = {
  argv: string[];
  cwd: string;
  env: Record<string, string>;
  stdin: string;
  deadlineMs: number;
  maxStdoutBytes: number;
  signal?: AbortSignal;
};

export interface BashRunner {
  run(args: BashRunArgs): Promise<RawProcessResult>;
}
