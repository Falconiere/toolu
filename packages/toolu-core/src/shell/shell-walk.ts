/**
 * The exhaustive command walk (#284). One `switch` over every unbash `Node`
 * kind yields each simple command once, with the context a gate needs: where
 * it runs (`origin`, `depth`, pipeline position) and whether a zero exit status
 * of the whole line proves it passed (`exitProves`).
 *
 * `exitProves` travels down as `proves`. It survives only on the last
 * statement of a list (not backgrounded), on an and-or element with `&&` on both
 * sides, on the last element of a non-negated pipeline, and into subshell/group
 * bodies and `bash -c`/`eval` strings. Conditions, loops, case arms, function
 * bodies and substitutions never prove anything.
 */
import {
  parse,
  type AndOr,
  type ArithmeticFor,
  type Case,
  type Command,
  type For,
  type If,
  type Node,
  type ParsedScript,
  type Pipeline,
  type Redirect,
  type Select,
  type Statement,
  type While,
} from "unbash";
import { runTarget, unwrap, type RunTarget } from "./shell-argv.ts";
import {
  unreachable,
  type CommandOrigin,
  type PipelinePosition,
  type ShellCommand,
  type ShellError,
  type ShellRedirect,
} from "./shell-types.ts";
import {
  visitArithmetic,
  visitAssignment,
  visitRedirect,
  visitTest,
  visitWord,
  type ScriptVisitor,
} from "./shell-walk-words.ts";
import { staticWord, toShellRedirect } from "./shell-words.ts";

/** How deep `bash -c` / `eval` strings are followed before the rest is unknown. */
export const MAX_RUN_DEPTH = 4;

const ALONE: PipelinePosition = { index: 0, size: 1 };

export interface WalkContext {
  /** The text positions index: the line, an inner `-c` string, or a decoded backtick body. */
  readonly source: string;
  readonly origin: CommandOrigin;
  readonly depth: number;
  readonly proves: boolean;
  readonly pipeline: PipelinePosition;
}

export interface WalkSink {
  readonly commands: ShellCommand[];
  readonly compoundRedirects: ShellRedirect[];
  readonly errors: ShellError[];
}

function unknownCommand(text: string, origin: CommandOrigin, depth: number, sink: WalkSink): void {
  sink.commands.push({
    words: [null],
    argv: [null],
    wrappers: [],
    redirects: [],
    pipeline: ALONE,
    exitProves: false,
    origin,
    depth,
    text,
  });
}

/** Walk a nested script (a substitution): it runs, but its status proves nothing. */
function nestedVisitor(ctx: WalkContext, sink: WalkSink): ScriptVisitor {
  return (script, text) => {
    if (script === undefined) {
      unknownCommand(text, "substitution", ctx.depth, sink);
      return;
    }
    const source = script.source ?? ctx.source;
    walkScript(
      script,
      { source, origin: "substitution", depth: ctx.depth, proves: false, pipeline: ALONE },
      sink,
    );
  };
}

function compoundRedirects(redirects: readonly Redirect[], ctx: WalkContext, sink: WalkSink): void {
  for (const redirect of redirects) {
    visitRedirect(redirect, nestedVisitor(ctx, sink));
    sink.compoundRedirects.push(toShellRedirect(redirect));
  }
}

/** The script a shell reads on stdin: a static heredoc or herestring, else unknown. */
function stdinScript(redirects: readonly ShellRedirect[]): string | null {
  for (const redirect of redirects) {
    if (redirect.fd !== null && redirect.fd !== 0) continue;
    if (redirect.heredoc !== null) return redirect.heredoc.content;
    if (redirect.operator === "<<<") return redirect.target;
  }
  return null;
}

function runString(
  target: RunTarget,
  redirects: readonly ShellRedirect[],
  text: string,
  ctx: WalkContext,
  sink: WalkSink,
): void {
  const script = target.stdin ? stdinScript(redirects) : target.script;
  if (script === null || ctx.depth + 1 > MAX_RUN_DEPTH) {
    unknownCommand(text, target.origin, ctx.depth + 1, sink);
    return;
  }
  const inner = { ...ctx, source: script, origin: target.origin, depth: ctx.depth + 1 };
  walkScript(parse(script), inner, sink);
}

function emitCommand(command: Command, ctx: WalkContext, sink: WalkSink): void {
  const nested = nestedVisitor(ctx, sink);
  for (const assignment of command.prefix) visitAssignment(assignment, nested);
  visitWord(command.name, nested);
  for (const word of command.suffix) visitWord(word, nested);
  for (const redirect of command.redirects) visitRedirect(redirect, nested);

  const words = command.name === undefined ? [] : [command.name, ...command.suffix].map(staticWord);
  const { argv, wrappers } = unwrap(words);
  const redirects = command.redirects.map(toShellRedirect);
  const text = ctx.source.slice(command.pos, command.end);
  const exitProves = ctx.proves;
  sink.commands.push({
    words,
    argv,
    wrappers,
    redirects,
    pipeline: ctx.pipeline,
    exitProves,
    origin: ctx.origin,
    depth: ctx.depth,
    text,
  });

  const target = runTarget(argv);
  if (target !== null) runString(target, redirects, text, ctx, sink);
}

function walkPipeline(node: Pipeline, ctx: WalkContext, sink: WalkSink): void {
  const size = node.commands.length;
  node.commands.forEach((command, index) => {
    const proves = ctx.proves && node.negated !== true && index === size - 1;
    walkNode(
      command,
      { ...ctx, proves, pipeline: size > 1 ? { index, size } : ctx.pipeline },
      sink,
    );
  });
}

function walkAndOr(node: AndOr, ctx: WalkContext, sink: WalkSink): void {
  node.commands.forEach((command, index) => {
    const before = index === 0 ? "&&" : node.operators[index - 1];
    const after = node.operators.slice(index);
    const proves = ctx.proves && before === "&&" && after.every((op) => op === "&&");
    walkNode(command, { ...ctx, proves }, sink);
  });
}

function walkIf(node: If, ctx: WalkContext, sink: WalkSink): void {
  walkList(node.clause.commands, ctx, sink);
  walkList(node.then.commands, ctx, sink);
  if (node.else !== undefined) walkNode(node.else, ctx, sink);
}

export function walkList(statements: readonly Statement[], ctx: WalkContext, sink: WalkSink): void {
  const last = statements.length - 1;
  statements.forEach((statement, index) => {
    const proves = ctx.proves && index === last && statement.background !== true;
    compoundRedirects(statement.redirects, ctx, sink);
    walkNode(statement.command, { ...ctx, proves }, sink);
  });
}

/** Conditions, loops and case arms: they run, but never prove the line's status. */
function walkOff(
  node: If | For | Select | ArithmeticFor | While | Case,
  ctx: WalkContext,
  sink: WalkSink,
): void {
  const off = { ...ctx, proves: false };
  const nested = nestedVisitor(ctx, sink);
  switch (node.type) {
    case "If":
      walkIf(node, off, sink);
      break;
    case "For":
    case "Select":
      for (const word of node.wordlist) visitWord(word, nested);
      walkList(node.body.commands, off, sink);
      break;
    case "ArithmeticFor":
      for (const part of [node.initialize, node.test, node.update]) visitArithmetic(part, nested);
      walkList(node.body.commands, off, sink);
      break;
    case "While":
      walkList(node.clause.commands, off, sink);
      walkList(node.body.commands, off, sink);
      break;
    case "Case":
      visitWord(node.word, nested);
      for (const item of node.items) {
        for (const pattern of item.pattern) visitWord(pattern, nested);
        walkList(item.body.commands, off, sink);
      }
      break;
    default:
      unreachable(node);
  }
}

export function walkNode(node: Node, ctx: WalkContext, sink: WalkSink): void {
  switch (node.type) {
    case "Command":
      emitCommand(node, ctx, sink);
      break;
    case "Pipeline":
      walkPipeline(node, ctx, sink);
      break;
    case "AndOr":
      walkAndOr(node, ctx, sink);
      break;
    case "If":
    case "For":
    case "Select":
    case "ArithmeticFor":
    case "While":
    case "Case":
      walkOff(node, ctx, sink);
      break;
    case "Function":
      compoundRedirects(node.redirects, ctx, sink);
      walkNode(node.body, { ...ctx, proves: false, origin: "function" }, sink);
      break;
    case "Coproc":
      compoundRedirects(node.redirects, ctx, sink);
      walkNode(node.body, { ...ctx, proves: false }, sink);
      break;
    case "Subshell":
    case "BraceGroup":
      walkList(node.body.commands, ctx, sink);
      break;
    case "CompoundList":
      walkList(node.commands, ctx, sink);
      break;
    case "TestCommand":
      visitTest(node.expression, nestedVisitor(ctx, sink));
      break;
    case "ArithmeticCommand":
      visitArithmetic(node.expression, nestedVisitor(ctx, sink));
      break;
    case "Statement":
      walkList([node], ctx, sink);
      break;
    default:
      unreachable(node);
  }
}

/** Walk a parsed script, collecting its errors; unbash reports nested errors on the nested script. */
export function walkScript(script: ParsedScript, ctx: WalkContext, sink: WalkSink): void {
  for (const error of script.errors ?? []) {
    sink.errors.push({ message: error.message, pos: error.pos, origin: ctx.origin });
  }
  walkList(script.commands, ctx, sink);
}
