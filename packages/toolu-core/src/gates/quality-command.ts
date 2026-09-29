/**
 * Quality commands in a parsed command line (#259). gate-status records the
 * quality gate only for a command that actually runs one of these, never for
 * text that names one (#283 item 6). The forms are `gate-status.sh`'s regex,
 * read from each simple command's argv (wrappers already removed) instead of
 * from the line's text. The regex also caught them behind a package runner or
 * as a script given to `bash`/`sh`, so those stay recognised.
 *
 * Reading argv parts from the regex on a few genuine forms, each pinned in the
 * tests. Broader: a tool by path (`./node_modules/.bin/tsc`, `/usr/bin/cargo
 * test`), `cargo +toolchain`, a wrapper script by path (`/repo/tools/x/check.sh`).
 * Narrower: `yarn run <tool>`, a runner option that takes a value
 * (`npx -p typescript tsc`), and a script shell given options (`bash -x script`).
 */
import { basename } from "node:path";
import type { ShellAnalysis, ShellCommand } from "../shell/shell-types.ts";

export type QualityCommand = {
  readonly command: ShellCommand;
  /** The quality command as `gate-status.sh` names it, e.g. `bun run lint`. */
  readonly label: string;
};

type Argv = readonly (string | null)[];

const BUN_SCRIPTS: ReadonlySet<string> = new Set([
  "check",
  "check:fix",
  "check:duplication",
  "ts:check",
  "ts:check:fix",
  "rust:check",
  "rust:test",
  "check-types",
  "lint",
  "lint:fix",
  "format",
  "format:check",
  "format:fix",
  "build",
  "test",
]);
const CARGO: ReadonlySet<string> = new Set(["clippy", "test", "build", "nextest"]);
const JS_TOOLS: ReadonlySet<string> = new Set(["vitest", "jest", "tsc"]);
const TS_CHECK: ReadonlySet<string> = new Set(["./scripts/ts-check.sh", "scripts/ts-check.sh"]);
const WRAPPER_SCRIPT = /(?:^|\/)(tools\/[A-Za-z0-9_.-]+\/(?:check|test|format)\.sh)$/;
const PACKAGE_RUNNERS: ReadonlySet<string> = new Set(["npx", "bunx", "pnpx", "yarn"]);
const SCRIPT_SHELLS: ReadonlySet<string> = new Set(["bash", "sh"]);

function bunLabel(argv: Argv): string | undefined {
  const [, verb, script] = argv;
  if (verb === "test") return "bun test";
  return verb === "run" && typeof script === "string" && BUN_SCRIPTS.has(script)
    ? `bun run ${script}`
    : undefined;
}

function cargoLabel(argv: Argv): string | undefined {
  const toolchain = argv[1]?.startsWith("+") === true;
  const sub = argv[toolchain ? 2 : 1];
  return typeof sub === "string" && CARGO.has(sub) ? `cargo ${sub}` : undefined;
}

/** The label of `argv` run directly, or undefined when it is no quality command. */
function directLabel(argv: Argv): string | undefined {
  const name = argv[0];
  if (typeof name !== "string") return undefined;
  const script = WRAPPER_SCRIPT.exec(name)?.[1];
  if (script !== undefined) return script;
  if (TS_CHECK.has(name)) return name;
  const base = basename(name);
  if (JS_TOOLS.has(base)) return base;
  if (base === "bun") return bunLabel(argv);
  return base === "cargo" ? cargoLabel(argv) : undefined;
}

/** The first word at or after `from` that is not an option. */
function afterOptions(argv: Argv, from: number): Argv {
  const at = argv.findIndex((word, i) => i >= from && !(word?.startsWith("-") ?? false));
  return at === -1 ? [] : argv.slice(at);
}

/** The command a package runner or script shell runs for `argv`, or undefined. */
function runnerTarget(argv: Argv): Argv | undefined {
  const name = argv[0];
  if (typeof name !== "string") return undefined;
  const base = basename(name);
  if (PACKAGE_RUNNERS.has(base)) return afterOptions(argv, 1);
  if ((base === "bun" && argv[1] === "x") || (base === "pnpm" && argv[1] === "exec")) {
    return afterOptions(argv, 2);
  }
  const script = argv[1];
  return SCRIPT_SHELLS.has(base) && typeof script === "string" && !script.startsWith("-")
    ? argv.slice(1)
    : undefined;
}

function labelOf(command: ShellCommand): string | undefined {
  if (command.origin === "function") return undefined;
  const direct = directLabel(command.argv);
  if (direct !== undefined) return direct;
  const target = runnerTarget(command.argv);
  return target === undefined ? undefined : directLabel(target);
}

/** Every simple command in the line that runs a quality command, in execution order. */
export function qualityCommands(analysis: ShellAnalysis): QualityCommand[] {
  return analysis.commands.flatMap((command) => {
    const label = labelOf(command);
    return label === undefined ? [] : [{ command, label }];
  });
}
