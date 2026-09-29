/**
 * Words (#284). One pass over a word's parts both resolves its static value and
 * visits every nested script it can run. A word is static only when every part
 * is literal, so `"$(cat <<'EOF' … EOF)"`, the form agents write commit messages
 * in, resolves to its heredoc body while anything that expands at run time is
 * `null`. unbash exposes `parts` as a lazy getter, so each field is read by
 * name: a generic walker over `Object.keys` would see no expansions at all.
 */
import type {
  ArithmeticExpression,
  AssignmentPrefix,
  ParsedScript,
  Redirect,
  TestExpression,
  Word,
  WordPart,
} from "unbash";
import { unreachable, type ShellRedirect } from "./shell-types.ts";

/** Receives each nested script; `undefined` when unbash left it unresolved. `text` is its source. */
export type ScriptVisitor = (script: ParsedScript | undefined, text: string) => void;

const IGNORE: ScriptVisitor = () => undefined;

/** A heredoc body's text as the command reads it, or `null` when it expands at run time. */
function heredocContent(redirect: Redirect): string | null {
  const content = redirect.content ?? "";
  // An unquoted body expands `$`, backticks and backslash escapes: static only without them.
  if (redirect.heredocQuoted !== true && /[$`\\]/.test(content)) return null;
  return redirect.operator === "<<-" ? content.replace(/^\t+/gm, "") : content;
}

function isHeredoc(redirect: Redirect): boolean {
  return redirect.operator === "<<" || redirect.operator === "<<-";
}

/** `$(cat <<TAG … TAG)`: the body minus trailing newlines, as command substitution yields it. */
function heredocCat(script: ParsedScript | undefined): string | null {
  if (script === undefined || (script.errors?.length ?? 0) > 0) return null;
  const [statement, ...others] = script.commands;
  if (statement === undefined || others.length > 0 || statement.background === true) return null;
  const command = statement.command;
  if (command.type !== "Command" || statement.redirects.length > 0) return null;
  if (command.prefix.length > 0 || command.suffix.length > 0) return null;
  if (scanWord(command.name, IGNORE).value !== "cat") return null;
  const [redirect, ...more] = command.redirects;
  if (redirect === undefined || more.length > 0 || !isHeredoc(redirect)) return null;
  return heredocContent(redirect)?.replace(/\n+$/, "") ?? null;
}

/** Visit a part's nested scripts; return its static value, or `null` when it expands. */
function scanPart(part: WordPart, visit: ScriptVisitor, quoted: boolean): string | null {
  switch (part.type) {
    case "Literal":
    case "SingleQuoted":
    case "AnsiCQuoted":
      return part.value;
    case "DoubleQuoted":
    case "LocaleString":
      return scanParts(part.parts, visit, true);
    case "SimpleExpansion":
      return null;
    case "ParameterExpansion":
      scanParts(part.indexParts, visit, false);
      for (const word of [part.operand, part.slice?.offset, part.slice?.length])
        scanWord(word, visit);
      for (const word of [part.replace?.pattern, part.replace?.replacement]) scanWord(word, visit);
      return null;
    case "CommandExpansion":
      visit(part.script, part.text);
      // Only a quoted `"$(cat <<'EOF' … EOF)"` is one static word; unquoted, it splits.
      return quoted ? heredocCat(part.script) : null;
    case "ProcessSubstitution":
      visit(part.script, part.text);
      return null;
    case "ArithmeticExpansion":
      visitArithmetic(part.expression, visit);
      return null;
    case "ExtendedGlob":
    case "BraceExpansion":
      scanParts(part.parts, visit, false);
      return part.type === "ExtendedGlob" ? part.text : null;
    default:
      return unreachable(part);
  }
}

/** Every part is scanned, so nested scripts are visited even after the value turns dynamic. */
function scanParts(
  parts: readonly WordPart[] | undefined,
  visit: ScriptVisitor,
  quoted: boolean,
): string | null {
  let value: string | null = "";
  for (const part of parts ?? []) {
    const piece = scanPart(part, visit, quoted);
    value = value === null || piece === null ? null : value + piece;
  }
  return value;
}

/** Unquoted `*`, `?` or `[…]`: bash replaces the word with the existing file it matches. */
function hasGlob(raw: string): boolean {
  const unescaped = raw.replace(/\\./gs, "");
  return /[*?]/.test(unescaped) || /\[[^\]]*\]/.test(unescaped);
}

export interface ResolvedWord {
  /** The value when every part is literal and nothing globs, else `null`. */
  readonly value: string | null;
  /** For a word whose only expansion is pathname globbing, its unexpanded pattern. */
  readonly pattern: string | null;
  /** Quotes removed, expansions left as written (`"$HOME/.env"` is `$HOME/.env`). */
  readonly text: string;
}

/**
 * Resolve a word statically and visit the scripts it runs. A pathname pattern
 * (`.en[v]`, `*.log`) is not a value: bash expands it to the one existing file
 * it matches, or keeps it as written when none does. So `value` is `null` and
 * `pattern` carries it.
 */
export function scanWord(word: Word | undefined, visit: ScriptVisitor): ResolvedWord {
  if (word === undefined) return { value: null, pattern: null, text: "" };
  const { parts, value: text } = word;
  const value = parts === undefined ? text : scanParts(parts, visit, false);
  const glob =
    parts === undefined
      ? hasGlob(word.text)
      : parts.some((p) => p.type === "ExtendedGlob" || (p.type === "Literal" && hasGlob(p.text)));
  return glob && value !== null
    ? { value: null, pattern: value, text }
    : { value, pattern: null, text };
}

export function visitArithmetic(
  expression: ArithmeticExpression | undefined,
  visit: ScriptVisitor,
): void {
  if (expression === undefined) return;
  switch (expression.type) {
    case "ArithmeticBinary":
      visitArithmetic(expression.left, visit);
      visitArithmetic(expression.right, visit);
      break;
    case "ArithmeticUnary":
      visitArithmetic(expression.operand, visit);
      break;
    case "ArithmeticTernary":
      visitArithmetic(expression.test, visit);
      visitArithmetic(expression.consequent, visit);
      visitArithmetic(expression.alternate, visit);
      break;
    case "ArithmeticGroup":
      visitArithmetic(expression.expression, visit);
      break;
    case "ArithmeticWord":
      scanParts(expression.parts, visit, false);
      break;
    case "ArithmeticCommandExpansion":
      visit(expression.script, expression.text);
      break;
    default:
      unreachable(expression);
  }
}

export function visitTest(expression: TestExpression, visit: ScriptVisitor): void {
  switch (expression.type) {
    case "TestUnary":
      scanWord(expression.operand, visit);
      break;
    case "TestBinary":
      scanWord(expression.left, visit);
      scanWord(expression.right, visit);
      break;
    case "TestLogical":
      visitTest(expression.left, visit);
      visitTest(expression.right, visit);
      break;
    case "TestNot":
      visitTest(expression.operand, visit);
      break;
    case "TestGroup":
      visitTest(expression.expression, visit);
      break;
    default:
      unreachable(expression);
  }
}

export function visitAssignment(assignment: AssignmentPrefix, visit: ScriptVisitor): void {
  scanParts(assignment.indexParts, visit, false);
  scanWord(assignment.value, visit);
  for (const word of assignment.array ?? []) scanWord(word, visit);
}

/** A redirect's target can run a substitution, and an unquoted heredoc body expands its own. */
export function toShellRedirect(redirect: Redirect, visit: ScriptVisitor): ShellRedirect {
  const heredoc = isHeredoc(redirect);
  const target = scanWord(redirect.target, visit);
  if (redirect.heredocQuoted !== true) scanWord(redirect.body, visit);
  return {
    operator: redirect.operator,
    fd: redirect.fileDescriptor ?? null,
    target: heredoc ? null : target.value,
    pattern: heredoc ? null : target.pattern,
    text: heredoc ? "" : target.text,
    heredoc: heredoc
      ? { content: heredocContent(redirect), quoted: redirect.heredocQuoted === true }
      : null,
  };
}
