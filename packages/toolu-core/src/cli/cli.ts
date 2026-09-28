/** Exit-status plumbing and argv helpers shared by toolu's skill CLIs (#270). */

/** Ends a CLI with `code`: `stdout` is written first, then a non-empty `message` to stderr. */
export class CliExit extends Error {
  readonly code: number;
  readonly stdout: string;

  constructor(code: number, message = "", stdout = "") {
    super(message);
    this.name = "CliExit";
    this.code = code;
    this.stdout = stdout;
  }
}

/** The value after `argv[index]`, or exit 1 naming the flag that lacks one. */
export function flagValue(tool: string, argv: readonly string[], index: number): string {
  const value = argv[index + 1];
  if (value === undefined) throw new CliExit(1, `${tool}: ${argv[index] ?? ""} needs a value`);
  return value;
}

/** `text` as a finite number, or exit 1 before any request is sent. */
export function numberValue(tool: string, flag: string, text: string): number {
  const value = Number(text);
  if (text.trim() === "" || !Number.isFinite(value)) {
    throw new CliExit(1, `${tool}: ${flag} must be a number`);
  }
  return value;
}

/** Writes `text` to stdout and waits until it is flushed, so a following exit cannot truncate it. */
export async function writeStdout(text: string): Promise<void> {
  if (text !== "") await Bun.write(Bun.stdout, text);
}

async function writeStderr(text: string): Promise<void> {
  if (text !== "") await Bun.write(Bun.stderr, text.endsWith("\n") ? text : `${text}\n`);
}

/**
 * Runs `main` and exits with its status. A thrown CliExit exits with its own
 * code after writing its stdout and message; anything else exits 1.
 */
export async function runCli(main: () => Promise<number>): Promise<never> {
  let code: number;
  try {
    code = await main();
  } catch (error) {
    if (error instanceof CliExit) {
      await writeStdout(error.stdout);
      await writeStderr(error.message);
      code = error.code;
    } else {
      await writeStderr(error instanceof Error ? error.message : String(error));
      code = 1;
    }
  }
  process.exit(code);
}
