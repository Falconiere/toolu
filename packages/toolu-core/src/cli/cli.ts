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

/** A JSON number as `jq --argjson` reads one: ASCII space around it, a leading `+` or `0`, a trailing `.`. */
const JQ_NUMBER = /^[ \t\n\r]*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?[ \t\n\r]*$/;

/**
 * `text` as a number, or exit 2 before any request is sent: the bash wrappers
 * fed it to `jq --argjson`, whose rejection ended the script with status 2.
 */
export function numberValue(tool: string, flag: string, text: string): number {
  const value = Number(text);
  // A finite check too: `1e999` is Infinity here, which JSON would send as null.
  if (!JQ_NUMBER.test(text) || !Number.isFinite(value)) {
    throw new CliExit(2, `${tool}: ${flag} must be a number`);
  }
  return value;
}

function isBrokenPipe(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "EPIPE";
}

/**
 * Writes `text` to stdout and waits until it is flushed, so a following exit
 * cannot truncate it. A reader that went away (`| head`) throws a silent
 * CliExit(141), the status a shell script got from SIGPIPE.
 */
export async function writeStdout(text: string): Promise<void> {
  if (text === "") return;
  try {
    await Bun.write(Bun.stdout, text);
  } catch (error) {
    if (isBrokenPipe(error)) throw new CliExit(141);
    throw error;
  }
}

async function writeStderr(text: string): Promise<void> {
  if (text !== "") await Bun.write(Bun.stderr, text.endsWith("\n") ? text : `${text}\n`);
}

/**
 * Writes a CliExit's stdout then its message and returns its code, or 141 when
 * the stdout reader is gone: the message still reaches stderr either way.
 */
async function report(exit: CliExit): Promise<number> {
  let code = exit.code;
  try {
    await writeStdout(exit.stdout);
  } catch (error) {
    if (!(error instanceof CliExit)) throw error;
    code = error.code;
  }
  await writeStderr(exit.message);
  return code;
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
      code = await report(error);
    } else {
      await writeStderr(error instanceof Error ? error.message : String(error));
      code = 1;
    }
  }
  process.exit(code);
}
