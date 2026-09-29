/**
 * Records `@toolu/core/shell` produces (#284). A consumer reads these, never the
 * unbash AST: every question a gate asks (what runs, what it writes, which git
 * operation, whether the exit status is observable) is a pure function over them.
 *
 * A `null` word is dynamic: its value depends on an expansion that only runs at
 * execution time. It is never guessed, so `$g push` has an unknown command name
 * rather than "not git".
 */
import type { RedirectOperator } from "unbash";

/** The innermost context a command runs in. */
export type CommandOrigin =
  /** Directly in the command line. */
  | "line"
  /** Inside `$(…)`, backticks, `<(…)`/`>(…)`, or an unquoted heredoc body. */
  | "substitution"
  /** Inside `bash -c STRING` (or sh/zsh/dash/ksh), or a static heredoc fed to a shell. */
  | "shell"
  /** Inside `eval ARGS`. */
  | "eval"
  /** Inside a function body: defined here, run only when called. */
  | "function";

/** An answer that a dynamic command name or subcommand can leave open. */
export type Tristate = "yes" | "no" | "unknown";

export interface ShellRedirect {
  readonly operator: RedirectOperator;
  /** The explicit descriptor (`2>`), or `null` when none was written. */
  readonly fd: number | null;
  /** The target's static value; `null` when dynamic or absent (a heredoc). */
  readonly target: string | null;
  /** The target as written; empty for a heredoc. */
  readonly text: string;
  /** A `<<`/`<<-` body: its content when static (tabs stripped for `<<-`), else `null`. */
  readonly heredoc: { readonly content: string | null; readonly quoted: boolean } | null;
}

export interface PipelinePosition {
  readonly index: number;
  readonly size: number;
}

export interface ShellCommand {
  /** Name and arguments as written, before unwrapping. */
  readonly words: readonly (string | null)[];
  /** The command that actually runs, after wrappers; xargs appends a trailing `null`. */
  readonly argv: readonly (string | null)[];
  /** Wrappers peeled off, outermost first (`sudo`, `timeout`, …). */
  readonly wrappers: readonly string[];
  readonly redirects: readonly ShellRedirect[];
  /** Position in the enclosing pipeline; `{ index: 0, size: 1 }` outside one. */
  readonly pipeline: PipelinePosition;
  /** An exit status of 0 for the whole line proves this command ran and exited 0. */
  readonly exitProves: boolean;
  readonly origin: CommandOrigin;
  /** `bash -c` / `eval` nesting depth; 0 on the line itself. */
  readonly depth: number;
  /** Source text of the command, in the script it was parsed from. */
  readonly text: string;
}

export interface ShellError {
  readonly message: string;
  /** Offset in the script the error was reported on (the line, or an inner `-c` string). */
  readonly pos: number;
  readonly origin: CommandOrigin;
}

export interface ShellAnalysis {
  readonly source: string;
  /** Every simple command, in execution order (substitutions before their command). */
  readonly commands: readonly ShellCommand[];
  /** Redirects on compound commands: `{ …; } >f`, `( … ) >f`, loops, function definitions. */
  readonly compoundRedirects: readonly ShellRedirect[];
  /** Errors from the line and from every nested script. */
  readonly errors: readonly ShellError[];
  /** Nothing is known: the input was oversize, or it had errors and no command at all. */
  readonly unknown: boolean;
}

/** Exhaustiveness guard for a `switch` over a union: a new member fails `tsc` here. */
export function unreachable(value: never): never {
  throw new Error(`@toolu/core/shell: unhandled node ${JSON.stringify(value)}`);
}
