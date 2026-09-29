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
 * bodies, substitutions and commands run by xargs (which may run them zero
 * times and still exit 0) never prove anything.
 */
import {
  parse,
  type AndOr,
  type Command,
  type Node,
  type ParsedScript,
  type Pipeline,
  type Redirect,
  type Statement,
} from "unbash";
import { alignUnwrapped, runTarget, unwrap, type RunTarget } from "./shell-argv.ts";
import {
  unreachable,
  type CommandOrigin,
  type PipelinePosition,
  type ShellCommand,
  type ShellError,
  type ShellRedirect,
} from "./shell-types.ts";
import {
  scanWord,
  stdinScript,
  toShellRedirect,
  visitArithmetic,
  visitAssignment,
  visitTest,
  type ScriptVisitor,
} from "./shell-words.ts";

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
    patterns: [null],
    texts: [text],
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
    sink.compoundRedirects.push(toShellRedirect(redirect, nestedVisitor(ctx, sink)));
  }
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
  const named = command.name === undefined ? [] : [command.name, ...command.suffix];
  const resolved = named.map((word) => scanWord(word, nested));
  const redirects = command.redirects.map((redirect) => toShellRedirect(redirect, nested));
  const words = resolved.map((word) => word.value);
  const unwrapped = unwrap(words);
  const argv = alignUnwrapped(words, unwrapped, null);
  const text = ctx.source.slice(command.pos, command.end);
  // xargs may run the command zero times and still exit 0.
  const proves = ctx.proves && !unwrapped.wrappers.includes("xargs");
  sink.commands.push({
    words,
    argv,
    patterns: alignUnwrapped(
      resolved.map((word) => word.pattern),
      unwrapped,
      null,
    ),
    texts: alignUnwrapped(
      resolved.map((word) => word.text),
      unwrapped,
      "",
    ),
    wrappers: unwrapped.wrappers,
    redirects,
    pipeline: ctx.pipeline,
    exitProves: proves,
    origin: ctx.origin,
    depth: ctx.depth,
    text,
  });

  const target = runTarget(argv);
  if (target !== null) runString(target, redirects, text, { ...ctx, proves }, sink);
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

export function walkList(statements: readonly Statement[], ctx: WalkContext, sink: WalkSink): void {
  const last = statements.length - 1;
  statements.forEach((statement, index) => {
    const proves = ctx.proves && index === last && statement.background !== true;
    compoundRedirects(statement.redirects, ctx, sink);
    walkNode(statement.command, { ...ctx, proves }, sink);
  });
}

/**
 * One case per unbash node kind. Conditions, loops, case arms, function bodies
 * and coprocs run under `off`: they never prove the line's status.
 */
export function walkNode(node: Node, ctx: WalkContext, sink: WalkSink): void {
  const nested = nestedVisitor(ctx, sink);
  const off = { ...ctx, proves: false };
  const list = (statements: readonly Statement[], at = off) => walkList(statements, at, sink);
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
      for (const part of [node.clause, node.then]) list(part.commands);
      if (node.else !== undefined) walkNode(node.else, off, sink);
      break;
    case "For":
    case "Select":
      for (const word of node.wordlist) scanWord(word, nested);
      list(node.body.commands);
      break;
    case "ArithmeticFor":
      for (const part of [node.initialize, node.test, node.update]) visitArithmetic(part, nested);
      list(node.body.commands);
      break;
    case "While":
      for (const part of [node.clause, node.body]) list(part.commands);
      break;
    case "Case":
      scanWord(node.word, nested);
      for (const item of node.items) {
        for (const pattern of item.pattern) scanWord(pattern, nested);
        list(item.body.commands);
      }
      break;
    case "Function":
    case "Coproc":
      compoundRedirects(node.redirects, ctx, sink);
      walkNode(node.body, node.type === "Function" ? { ...off, origin: "function" } : off, sink);
      break;
    case "Subshell":
    case "BraceGroup":
    case "CompoundList":
      list(node.type === "CompoundList" ? node.commands : node.body.commands, ctx);
      break;
    case "TestCommand":
      visitTest(node.expression, nested);
      break;
    case "ArithmeticCommand":
      visitArithmetic(node.expression, nested);
      break;
    case "Statement":
      list([node], ctx);
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
