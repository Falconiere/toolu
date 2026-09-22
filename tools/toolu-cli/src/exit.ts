/** Process exit codes. Documented in docs/toolu/specs/2026-09-22-npx-toolu-cli-design.md. */
export const EXIT = {
  ok: 0,
  failed: 1,
  usage: 2,
  missingInput: 3,
  cancelled: 130,
} as const;

export type ExitCode = (typeof EXIT)[keyof typeof EXIT];

/** An error carrying the exit code the process should report. */
export class CliError extends Error {
  override name = "CliError";
  readonly code: ExitCode;

  constructor(code: ExitCode, message: string) {
    super(message);
    this.code = code;
  }
}

export class UsageError extends CliError {
  override name = "UsageError";
  constructor(message: string) {
    super(EXIT.usage, message);
  }
}
