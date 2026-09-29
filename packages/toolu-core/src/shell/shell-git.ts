/**
 * Git operations in a command line (#284): the subcommand past git's global
 * options, the cumulative `-C` chain and refspec destination of a push, and the
 * static `-m` messages of a commit.
 */
import { basename } from "node:path";
import { optionValues, parseArgs } from "./shell-options.ts";
import type { ShellAnalysis, ShellCommand, Tristate } from "./shell-types.ts";

export interface GitInvocation {
  readonly command: ShellCommand;
  /** The subcommand, or `null` when a dynamic word stands where it would be. */
  readonly subcommand: string | null;
  /** Words after the subcommand. */
  readonly args: readonly (string | null)[];
  /** Every `-C` value in order; git applies each relative to the previous one. */
  readonly cChain: readonly (string | null)[];
}

const GLOBALS = {
  valueShort: "Cc",
  valueLong: "git-dir work-tree namespace super-prefix config-env attr-source",
  stopAtOperand: true,
};

/** The git invocation `command` runs, or `undefined` when it is not git, malformed, or has no subcommand. */
export function gitInvocation(command: ShellCommand): GitInvocation | undefined {
  const name = command.argv[0];
  if (name === null || name === undefined || basename(name) !== "git") return undefined;
  const globals = parseArgs(command.argv, 1, GLOBALS);
  if (globals.missingValue || globals.next >= command.argv.length) return undefined;
  const cChain = optionValues(globals, "C").map((value) => value ?? null);
  const subcommand = command.argv[globals.next] ?? null;
  return { command, subcommand, args: command.argv.slice(globals.next + 1), cChain };
}

/**
 * Whether the line runs `git <sub>`. `unknown` when it cannot be ruled out: the
 * analysis is unknown, a command name is dynamic, or a git subcommand is.
 */
export function runsGitSubcommand(analysis: ShellAnalysis, sub: string): Tristate {
  let unknown = analysis.unknown;
  for (const command of analysis.commands) {
    if (command.argv[0] === null) {
      unknown = true;
      continue;
    }
    const git = gitInvocation(command);
    if (git?.subcommand === sub) return "yes";
    if (git?.subcommand === null) unknown = true;
  }
  return unknown ? "unknown" : "no";
}

export interface GitPush {
  readonly invocation: GitInvocation;
  readonly cChain: readonly (string | null)[];
  /** The refspec (second positional after `push`); `undefined` when absent, `null` when dynamic. */
  readonly refspec: string | null | undefined;
  /** The branch the refspec pushes to, when it names exactly one. */
  readonly destination: string | null;
}

const PUSH_OPTIONS = {
  valueShort: "o",
  valueLong: "push-option receive-pack exec repo",
};

/** `HEAD:x`, `+HEAD:refs/heads/x`, `src:x` and bare `x` name `x`; a delete, bare `HEAD` or a wildcard names none. */
function destinationOf(refspec: string | null | undefined): string | null {
  if (refspec === null || refspec === undefined) return null;
  const spec = refspec.startsWith("+") ? refspec.slice(1) : refspec;
  if (spec.startsWith(":") || spec === "HEAD") return null;
  const colon = spec.indexOf(":");
  const dst = (colon === -1 ? spec : spec.slice(colon + 1)).replace(/^refs\/heads\//, "");
  return dst === "" || dst.includes("*") ? null : dst;
}

/** Every `git push` in the line, in execution order. */
export function pushTargets(analysis: ShellAnalysis): GitPush[] {
  return analysis.commands.flatMap((command) => {
    const invocation = gitInvocation(command);
    if (invocation?.subcommand !== "push") return [];
    const refspec = parseArgs(invocation.args, 0, PUSH_OPTIONS).operands[1];
    const destination = destinationOf(refspec);
    return [{ invocation, cChain: invocation.cChain, refspec, destination }];
  });
}

const COMMIT_OPTIONS = {
  valueShort: "mFCct",
  restShort: "Su",
  valueLong:
    "message file reuse-message reedit-message template author date cleanup fixup squash trailer pathspec-from-file",
};

/** The `-m`/`--message` values of a commit, in order; `null` for a dynamic one. */
export function commitMessages(invocation: GitInvocation): readonly (string | null)[] {
  if (invocation.subcommand !== "commit") return [];
  const parsed = parseArgs(invocation.args, 0, COMMIT_OPTIONS);
  return optionValues(parsed, "m message").map((value) => value ?? null);
}
