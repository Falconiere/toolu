/**
 * Context tests for the script lexer: where a `/` starts a regex literal, where
 * `(` opens a control-statement head, and how comments are skipped as trivia.
 * Callers pass code-only context (strings, regexes and JSX collapse to a value
 * token), so a trailing `*\/` never turns a division into a regex.
 */
import { at, isMemberChar, isIdent, isSpace, sub, trimEnd } from "./chars.ts";

const OPERATOR_ENDINGS = [
  "=>", "=", "(", "[", "{", ",", ":", "?", ";", "!", "~", "+", "-", "*", "/", "%", "&", "|", "^",
];
const REGEX_KEYWORDS = [
  "return", "case", "delete", "void", "typeof", "yield", "await", "throw", "else", "do",
  "instanceof", "in", "of",
];
const CONTROL_KEYWORDS = ["if", "while", "for", "with"];

/** `prefix` ends with one of `keywords` as a whole token (not `x.if`, `#if`, `elif`). */
export function afterKeyword(prefix: string, keywords: readonly string[]): boolean {
  const trimmed = trimEnd(prefix);
  for (const keyword of keywords) {
    if (!trimmed.endsWith(keyword)) continue;
    const before = trimEnd(trimmed.slice(0, trimmed.length - keyword.length));
    if (!isMemberChar(before.slice(-1))) return true;
  }
  return false;
}

export function isControlParen(prefix: string): boolean {
  return afterKeyword(prefix, CONTROL_KEYWORDS);
}

export function endsWithOperator(prefix: string, extra: readonly string[] = []): boolean {
  return prefix === "" || [...extra, ...OPERATOR_ENDINGS].some((ending) => prefix.endsWith(ending));
}

/** Division follows a completed value; a regex literal follows an expression boundary. */
export function startsRegex(line: string, offset: number): boolean {
  if (at(line, offset + 1) === "=") return false;
  const prefix = trimEnd(line.slice(0, offset));
  if (prefix.endsWith("++") || prefix.endsWith("--")) return false;
  if (prefix.endsWith("!")) {
    const last = prefix.slice(0, -1).slice(-1);
    if (isIdent(last) || [")", "]", "}", '"', "'", "`"].includes(last)) return false;
  }
  return afterKeyword(prefix, REGEX_KEYWORDS) || endsWithOperator(prefix);
}

/** Index after whitespace and comments starting at `start`. */
export function skipTrivia(line: string, start: number): number {
  let i = start;
  while (i < line.length) {
    const pair = at(line, i) + at(line, i + 1);
    if (isSpace(at(line, i))) {
      i += 1;
    } else if (pair === "/*") {
      i += 2;
      while (i < line.length && sub(line, i, 2) !== "*/") i += 1;
      i += 2;
    } else if (pair === "//") {
      while (i < line.length && at(line, i) !== "\n") i += 1;
    } else {
      break;
    }
  }
  return i;
}
