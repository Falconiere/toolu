/**
 * Rust lexer: keep code, blank comments, strings, raw strings and
 * character/byte literals. Block comments nest and raw-string delimiters carry
 * any number of hashes, so both persist across physical lines.
 */
import { at, sub } from "./chars.ts";

export type RustState = { state: string; commentDepth: number; rawClose: string };

export function newRustState(): RustState {
  return { state: "code", commentDepth: 0, rawClose: "" };
}

type Cursor = { line: string; i: number; out: string };

function blankTo(c: Cursor, last: number): void {
  while (c.i <= last) {
    c.out += " ";
    c.i += 1;
  }
}

/** `'x'` / `'\n'`: a char literal when a closing quote follows; otherwise a lifetime. */
function charLiteral(c: Cursor): void {
  let probe = c.i + 1;
  if (at(c.line, probe) === "\\") {
    while (probe < c.line.length) {
      if (at(c.line, probe) === "\\") probe += 2;
      else if (at(c.line, probe) === "'") break;
      else probe += 1;
    }
  } else {
    probe += 1;
  }
  if (probe < c.line.length && at(c.line, probe) === "'") {
    blankTo(c, probe);
  } else {
    c.out += "'";
    c.i += 1;
  }
}

/** `r"…"` / `r#"…"#`: enter a raw string when the quote follows the hashes. */
function rawString(st: RustState, c: Cursor): void {
  let probe = c.i + 1;
  let hashes = "";
  while (probe < c.line.length && at(c.line, probe) === "#") {
    hashes += "#";
    probe += 1;
  }
  if (probe < c.line.length && at(c.line, probe) === '"') {
    st.rawClose = `"${hashes}`;
    st.state = "raw";
    blankTo(c, probe);
  } else {
    c.out += "r";
    c.i += 1;
  }
}

function code(st: RustState, c: Cursor): void {
  const char = at(c.line, c.i);
  const pair = char + at(c.line, c.i + 1);
  if (pair === "//") {
    blankTo(c, c.line.length - 1);
  } else if (pair === "/*") {
    c.out += "  ";
    st.state = "block";
    st.commentDepth = 1;
    c.i += 2;
  } else if (char === "'") {
    charLiteral(c);
  } else if (char === '"') {
    c.out += " ";
    st.state = "string";
    c.i += 1;
  } else if (char === "r") {
    rawString(st, c);
  } else {
    c.out += char;
    c.i += 1;
  }
}

function block(st: RustState, c: Cursor): void {
  const pair = sub(c.line, c.i, 2);
  if (pair === "/*" || pair === "*/") {
    st.commentDepth += pair === "/*" ? 1 : -1;
    c.out += "  ";
    c.i += 2;
    if (st.commentDepth === 0) st.state = "code";
  } else {
    c.out += " ";
    c.i += 1;
  }
}

function literal(st: RustState, c: Cursor): void {
  if (st.state === "raw") {
    if (sub(c.line, c.i, st.rawClose.length) === st.rawClose) {
      blankTo(c, c.i + st.rawClose.length - 1);
      st.state = "code";
      st.rawClose = "";
    } else {
      c.out += " ";
      c.i += 1;
    }
    return;
  }
  const char = at(c.line, c.i);
  c.out += " ";
  c.i += 1;
  if (char === "\\" && c.i < c.line.length) {
    c.out += " ";
    c.i += 1;
  } else if (char === '"') {
    st.state = "code";
  }
}

export function rustSyntaxLine(st: RustState, line: string): string {
  const c: Cursor = { line, i: 0, out: "" };
  while (c.i < line.length) {
    if (st.state === "code") code(st, c);
    else if (st.state === "block") block(st, c);
    else literal(st, c);
  }
  return c.out;
}
