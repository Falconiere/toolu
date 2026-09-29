/**
 * The exhaustive walk below the command level (#284): every word part,
 * arithmetic node and `[[ ]]` expression that can hold a nested script. unbash
 * exposes `parts` as a lazy getter, so each field is read by name: a generic
 * walker over `Object.keys` would see no expansions at all.
 */
import type {
  ArithmeticExpression,
  AssignmentPrefix,
  ParameterExpansionPart,
  ParsedScript,
  Redirect,
  TestExpression,
  Word,
  WordPart,
} from "unbash";
import { unreachable } from "./shell-types.ts";

/** Receives each nested script; `undefined` when unbash left it unresolved. `text` is its source. */
export type ScriptVisitor = (script: ParsedScript | undefined, text: string) => void;

function visitParts(parts: readonly WordPart[] | undefined, visit: ScriptVisitor): void {
  for (const part of parts ?? []) visitPart(part, visit);
}

function visitParameter(part: ParameterExpansionPart, visit: ScriptVisitor): void {
  visitParts(part.indexParts, visit);
  visitWord(part.operand, visit);
  visitWord(part.slice?.offset, visit);
  visitWord(part.slice?.length, visit);
  visitWord(part.replace?.pattern, visit);
  visitWord(part.replace?.replacement, visit);
}

function visitPart(part: WordPart, visit: ScriptVisitor): void {
  switch (part.type) {
    case "Literal":
    case "SingleQuoted":
    case "AnsiCQuoted":
    case "SimpleExpansion":
      break;
    case "DoubleQuoted":
    case "LocaleString":
      visitParts(part.parts, visit);
      break;
    case "ParameterExpansion":
      visitParameter(part, visit);
      break;
    case "CommandExpansion":
    case "ProcessSubstitution":
      visit(part.script, part.text);
      break;
    case "ArithmeticExpansion":
      visitArithmetic(part.expression, visit);
      break;
    case "ExtendedGlob":
    case "BraceExpansion":
      visitParts(part.parts, visit);
      break;
    default:
      unreachable(part);
  }
}

export function visitWord(word: Word | undefined, visit: ScriptVisitor): void {
  visitParts(word?.parts, visit);
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
      visitParts(expression.parts, visit);
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
      visitWord(expression.operand, visit);
      break;
    case "TestBinary":
      visitWord(expression.left, visit);
      visitWord(expression.right, visit);
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

/** A redirect target can run a substitution; an unquoted heredoc body expands its own. */
export function visitRedirect(redirect: Redirect, visit: ScriptVisitor): void {
  visitWord(redirect.target, visit);
  if (redirect.heredocQuoted !== true) visitWord(redirect.body, visit);
}

export function visitAssignment(assignment: AssignmentPrefix, visit: ScriptVisitor): void {
  visitParts(assignment.indexParts, visit);
  visitWord(assignment.value, visit);
  for (const word of assignment.array ?? []) visitWord(word, visit);
}
