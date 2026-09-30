/**
 * Programs a command runs on its own behalf that `@toolu/core/shell` leaves as
 * plain arguments: `find … -exec <cmd>`, `watch <cmd>`, `npx <cmd>`,
 * `pnpm exec <cmd>`. bash's text match saw them, so the port must too.
 */
import { basename } from "node:path";
import type { ShellCommand } from "@toolu/core/shell";

const EXEC_FLAGS = new Set(["-exec", "-execdir", "-ok", "-okdir"]);
const RUNNERS = new Set(["npx", "bunx", "pnpx", "watch"]);
const RUNNER_VERBS = new Map([
  ["pnpm", new Set(["exec", "dlx"])],
  ["yarn", new Set(["exec", "dlx"])],
  ["npm", new Set(["exec"])],
  ["bun", new Set(["x"])],
]);

/** Index of the first argument at or after `from` that is not a flag. */
function firstOperand(argv: ShellCommand["argv"], from: number): number | undefined {
  const at = argv.findIndex((arg, i) => i >= from && !(arg ?? "").startsWith("-"));
  return at < 0 ? undefined : at;
}

function launchedIndex(argv: ShellCommand["argv"]): number | undefined {
  const name = basename(argv[0] ?? "");
  if (name === "find") {
    const flag = argv.findIndex((arg) => arg !== null && EXEC_FLAGS.has(arg));
    return flag < 0 ? undefined : flag + 1;
  }
  if (RUNNERS.has(name)) return firstOperand(argv, 1);
  const verb = argv[1];
  return verb !== null && verb !== undefined && RUNNER_VERBS.get(name)?.has(verb)
    ? firstOperand(argv, 2)
    : undefined;
}

/** `argv` indexes of the programs `command` runs: its own, then the one it launches, if any. */
export function programIndexes(command: ShellCommand): number[] {
  const launched = launchedIndex(command.argv);
  return launched === undefined || launched >= command.argv.length ? [0] : [0, launched];
}

/** The basename of the program at `index`, or undefined for a dynamic word. */
export function programAt(command: ShellCommand, index: number): string | undefined {
  const word = command.argv[index];
  return typeof word === "string" ? basename(word) : undefined;
}
