/**
 * TSX generic-arrow lookahead: `<T,>(x) => …`, `<const T>(…) =>`,
 * `<T = X>(…) =>`, `<T extends X>(…) =>` are type parameters, not JSX. TSX
 * requires an ambiguity breaker after the first name, so a plain `<pre>(x) =>`
 * stays JSX text. The scan runs over the whole source, so multiline headers,
 * comments and regex literals inside the parameter list are handled.
 */
import { at, isIdent, isIdentStart, isSpace, sub } from "./chars.ts";
import { isControlParen, skipTrivia, startsRegex } from "./js-context.ts";

/** After `<`: an optional `const`, a name, then `,` `=` or `extends`. */
function hasTypeParamBreaker(line: string, start: number): boolean {
  let i = skipTrivia(line, start);
  if (sub(line, i, 5) === "const") {
    const after = at(line, i + 5);
    if (isSpace(after) || sub(line, i + 5, 2) === "/*") i = skipTrivia(line, i + 5);
  }
  if (!isIdentStart(at(line, i))) return false;
  i += 1;
  while (i < line.length && isIdent(at(line, i))) i += 1;
  i = skipTrivia(line, i);
  const char = at(line, i);
  if (char === "," || char === "=") return true;
  return sub(line, i, 7) === "extends" && !isIdent(at(line, i + 7));
}

/** Index just past the `>` that balances the opening `<`, or -1. */
function closeAngle(line: string, start: number): number {
  let i = start;
  let quote = "";
  let mode = "code";
  let angle = 1;
  while (i < line.length && angle > 0) {
    const char = at(line, i);
    const pair = char + at(line, i + 1);
    if (mode === "code") {
      if (pair === "//") mode = "line_comment";
      else if (pair === "/*") mode = "block_comment";
      else if (char === "'" || char === '"' || char === "`") {
        quote = char;
        mode = "quote";
      } else if (char === "<") angle += 1;
      else if (char === ">" && at(line, i - 1) !== "=") angle -= 1;
    } else if (mode === "quote") {
      if (char === "\\") i += 1;
      else if (char === quote) mode = "code";
    } else if (mode === "block_comment") {
      if (pair === "*/") {
        mode = "code";
        i += 1;
      }
    } else if (char === "\n") {
      mode = "code";
    }
    i += 1;
  }
  return angle === 0 ? i : -1;
}

type ParamScan = {
  i: number;
  paren: number;
  mode: string;
  quote: string;
  regexClass: boolean;
  context: string;
  controls: string[];
  afterControl: boolean;
};

function paramCode(line: string, s: ParamScan): void {
  const char = at(line, s.i);
  const next = at(line, s.i + 1);
  if (char + next === "//") s.mode = "line_comment";
  else if (char + next === "/*") s.mode = "block_comment";
  else if (char === "/" && (s.afterControl || startsRegex(`${s.context}/${next}`, s.context.length))) {
    s.mode = "regex";
    s.regexClass = false;
    s.context += "v";
    s.afterControl = false;
  } else if (char === "'" || char === '"' || char === "`") {
    s.quote = char;
    s.mode = "quote";
    s.context += "v";
    s.afterControl = false;
  } else if (char === "(") {
    s.controls.unshift(isControlParen(s.context) ? "control" : "value");
    s.paren += 1;
    s.context += char;
    s.afterControl = false;
  } else if (char === ")") {
    const kind = s.controls.shift() ?? "";
    s.paren -= 1;
    s.context += char;
    s.afterControl = kind === "control";
  } else {
    s.context += char;
    if (!isSpace(char)) s.afterControl = false;
  }
}

function paramOther(line: string, s: ParamScan): void {
  const char = at(line, s.i);
  if (s.mode === "quote") {
    if (char === "\\") s.i += 1;
    else if (char === s.quote) s.mode = "code";
  } else if (s.mode === "block_comment") {
    if (char + at(line, s.i + 1) === "*/") {
      s.mode = "code";
      s.i += 1;
    }
  } else if (s.mode === "line_comment") {
    if (char === "\n") {
      s.mode = "code";
      s.context += " ";
    }
  } else if (char === "\\") {
    s.i += 1;
  } else if (s.regexClass) {
    if (char === "]") s.regexClass = false;
  } else if (char === "[") {
    s.regexClass = true;
  } else if (char === "/" || char === "\n") {
    s.mode = "code";
  }
}

/** Index just past the `)` closing the parameter list opened at `start - 1`, or -1. */
function closeParams(line: string, start: number): number {
  const s: ParamScan = {
    i: start,
    paren: 1,
    mode: "code",
    quote: "",
    regexClass: false,
    context: "(",
    controls: ["value"],
    afterControl: false,
  };
  while (s.i < line.length && s.paren > 0) {
    if (s.mode === "code") paramCode(line, s);
    else paramOther(line, s);
    s.i += 1;
  }
  return s.paren === 0 ? s.i : -1;
}

/** Does the `<` at `offset` open a generic arrow function's type parameters? */
export function isGenericArrow(line: string, offset: number): boolean {
  if (!hasTypeParamBreaker(line, offset + 1)) return false;
  const angleEnd = closeAngle(line, offset + 1);
  if (angleEnd === -1) return false;
  const open = skipTrivia(line, angleEnd);
  if (at(line, open) !== "(") return false;
  const paramsEnd = closeParams(line, open + 1);
  if (paramsEnd === -1) return false;
  return sub(line, skipTrivia(line, paramsEnd), 2) === "=>";
}
