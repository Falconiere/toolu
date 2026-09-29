/**
 * Script lexer (TS/JS/JSX/Astro): replace quoted, template, regex and rendered
 * JSX text with spaces while keeping comments and code byte-aligned, so a
 * directive-looking string is documentation while a real lint comment stays
 * visible. Template `${…}` and JSX `{…}` expressions return to code.
 */
import { at, isSpace, isTagStart, trimEnd } from "./chars.ts";
import { endsWithOperator, isControlParen, startsRegex } from "./js-context.ts";
import { isGenericArrow } from "./generic-arrow.ts";
import { finishJsxRoot, popBrace, pushBrace, startJsxRoot } from "./script-state.ts";
import type { ScriptState } from "./script-state.ts";

type Cursor = { line: string; i: number; out: string; context: string };

const JSX_KEYWORD_ENDINGS = ["return", "default", "yield", "await", "throw"];

/** A `<name` token is JSX only at an expression boundary and not as a generic arrow. */
function startsJsx(st: ScriptState, line: string, offset: number): boolean {
  if (!st.jsxEnabled) return false;
  const next = at(line, offset + 1);
  if (!(next === "/" || next === ">" || isTagStart(next))) return false;
  const source = st.source ?? line;
  const sourceOffset = st.source === undefined ? offset : st.sourceOffset + offset;
  if (isGenericArrow(source, sourceOffset)) return false;
  const prefix = trimEnd(line.slice(0, offset));
  return endsWithOperator(prefix, JSX_KEYWORD_ENDINGS);
}

function codeSymbol(st: ScriptState, c: Cursor, char: string): void {
  c.out += char;
  c.context += char;
  c.i += 1;
  if (char === "(") {
    st.parens.unshift(isControlParen(c.context.slice(0, -1)) ? "control" : "value");
    st.afterControl = false;
  } else if (char === ")") {
    const kind = st.parens.shift() ?? "";
    st.afterControl = kind === "control";
  } else if (char === "{") {
    if (st.braceDepth > 0) st.braceDepth += 1;
    st.afterControl = false;
  } else if (char === "}") {
    st.afterControl = false;
    if (st.braceDepth > 0) {
      st.braceDepth -= 1;
      if (st.braceDepth === 0) popBrace(st);
    }
  } else if (!isSpace(char)) {
    st.afterControl = false;
  }
}

function openQuote(st: ScriptState, c: Cursor, state: string): void {
  c.out += " ";
  c.context += "v";
  st.afterControl = false;
  st.state = state;
  c.i += 1;
}

function code(st: ScriptState, c: Cursor): void {
  const char = at(c.line, c.i);
  const next = at(c.line, c.i + 1);
  if (char + next === "//") {
    c.out += c.line.slice(c.i);
    c.i = c.line.length;
  } else if (char + next === "/*") {
    c.out += "/*";
    st.state = "block";
    c.i += 2;
  } else if (
    char === "/" &&
    (st.afterControl || startsRegex(`${c.context}/${next}`, c.context.length))
  ) {
    c.out += " ";
    st.state = "regex";
    st.regexClass = false;
    st.afterControl = false;
    c.context += "v";
    c.i += 1;
  } else if (char === "<" && startsJsx(st, c.line, c.i)) {
    c.out += " ";
    st.afterControl = false;
    c.context += "v";
    c.i += 1;
    if (next === "/") {
      c.out += " ";
      c.i += 1;
      startJsxRoot(st, "close");
    } else {
      startJsxRoot(st, "open");
    }
  } else if (char === "'") openQuote(st, c, "single");
  else if (char === '"') openQuote(st, c, "double");
  else if (char === "`") openQuote(st, c, "template");
  else codeSymbol(st, c, char);
}

/** Blank one character (and the one after a backslash) inside a literal. */
function blankEscaped(c: Cursor, char: string): boolean {
  c.out += " ";
  c.i += 1;
  if (char === "\\" && c.i < c.line.length) {
    c.out += " ";
    c.i += 1;
    return true;
  }
  return false;
}

function literal(st: ScriptState, c: Cursor): void {
  const char = at(c.line, c.i);
  const next = at(c.line, c.i + 1);
  if (st.state === "block") {
    c.out += char;
    c.i += 1;
    if (char + next === "*/") {
      c.out += next;
      st.state = "code";
      c.i += 1;
    }
    return;
  }
  if (blankEscaped(c, char)) return;
  if (st.state === "regex") {
    if (st.regexClass) {
      if (char === "]") st.regexClass = false;
    } else if (char === "[") st.regexClass = true;
    else if (char === "/") st.state = "code";
  } else if (st.state === "single" || st.state === "double") {
    if (char === (st.state === "single" ? "'" : '"')) st.state = "code";
  } else if (char + next === "${") {
    c.out += " ";
    c.context += "{";
    c.i += 1;
    pushBrace(st, "template");
  } else if (char === "`") {
    st.state = "code";
  }
}

function jsxText(st: ScriptState, c: Cursor, char: string, next: string): void {
  if (char === "<") {
    st.state = "jsx_tag";
    st.tagKind = "open";
    st.tagLast = "";
    if (next === "/") {
      c.out += " ";
      c.i += 1;
      st.tagKind = "close";
    }
  } else if (char === "{") {
    c.context += "{";
    pushBrace(st, "jsx_text");
  }
}

function jsxTagClose(st: ScriptState): void {
  if (st.tagKind === "close") {
    st.jsxDepth -= 1;
    if (st.jsxDepth === 0) finishJsxRoot(st);
    else st.state = "jsx_text";
  } else if (st.tagLast === "/") {
    if (st.jsxDepth === 0) finishJsxRoot(st);
    else st.state = "jsx_text";
  } else {
    st.jsxDepth += 1;
    st.state = "jsx_text";
  }
}

function jsx(st: ScriptState, c: Cursor): void {
  const char = at(c.line, c.i);
  const next = at(c.line, c.i + 1);
  c.out += " ";
  c.i += 1;
  if (st.state === "jsx_text") jsxText(st, c, char, next);
  else if (st.state === "jsx_single" || st.state === "jsx_double") {
    if (char === (st.state === "jsx_single" ? "'" : '"')) st.state = "jsx_tag";
  } else if (char === "'") st.state = "jsx_single";
  else if (char === '"') st.state = "jsx_double";
  else if (char === "{") {
    c.context += "{";
    pushBrace(st, "jsx_tag");
  } else if (char === ">") jsxTagClose(st);
  else if (!isSpace(char)) st.tagLast = char;
}

const LITERAL_STATES = new Set(["block", "regex", "single", "double", "template"]);
const JSX_STATES = new Set(["jsx_text", "jsx_tag", "jsx_single", "jsx_double"]);

/** Sanitize one physical line, carrying state to the next. */
export function scriptSyntaxLine(st: ScriptState, line: string): string {
  const c: Cursor = { line, i: 0, out: "", context: st.context };
  while (c.i < line.length) {
    if (st.state === "code") code(st, c);
    else if (LITERAL_STATES.has(st.state)) literal(st, c);
    else if (JSX_STATES.has(st.state)) jsx(st, c);
    else throw new Error(`lint-suppressions lexer reached unknown state "${st.state}"`);
  }
  // A regex literal cannot cross a newline: recover so one bad line cannot hide later directives.
  if (st.state === "regex") st.state = "code";
  const context = `${c.context} `;
  st.context = context.length > 128 ? context.slice(-128) : context;
  return c.out;
}
