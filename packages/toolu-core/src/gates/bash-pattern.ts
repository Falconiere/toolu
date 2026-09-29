/**
 * Bash pattern matching as `[[ text == $pattern ]]` does it with `extglob` on
 * (#260), the check `glob_match` ran for protected-files and code-edit-rules.
 * `*` and `?` cross `/`, a leading `.` is not special, and backslash escapes
 * the next character. Brackets take `!`/`^` negation, ranges and `[:class:]`.
 * `?( ) *( ) +( ) @( ) !( )` take `|`-separated pattern lists. Matching
 * backtracks over the parsed pattern, which is exact for `!( )`: bash accepts
 * a split whose prefix matches none of the listed patterns.
 */

type Bracket = { negated: boolean; test: (char: string) => boolean };
type ExtOp = "?" | "*" | "+" | "@" | "!";
type Node =
  | { kind: "literal"; char: string }
  | { kind: "any" }
  | { kind: "star" }
  | { kind: "bracket"; bracket: Bracket }
  | { kind: "ext"; op: ExtOp; alternatives: Node[][] };

const CLASSES: Readonly<Record<string, RegExp>> = {
  alnum: /[\p{L}\p{N}]/u,
  alpha: /\p{L}/u,
  blank: /[ \t]/,
  cntrl: /\p{Cc}/u,
  digit: /[0-9]/,
  graph: /[^\s\p{Cc}]/u,
  lower: /\p{Ll}/u,
  print: /[^\p{Cc}]/u,
  punct: /[!-/:-@[-`{-~]/,
  space: /\s/,
  upper: /\p{Lu}/u,
  word: /[\p{L}\p{N}_]/u,
  xdigit: /[0-9A-Fa-f]/,
};

/** One bracket member: a class, an equivalence/collating element, or a character. */
function bracketMember(
  chars: readonly string[],
  at: number,
): { next: number; test: (c: string) => boolean; char?: string } | undefined {
  const open = chars[at];
  const kind = chars[at + 1];
  if (open === "[" && (kind === ":" || kind === "=" || kind === ".")) {
    const close = chars.indexOf(kind, at + 2);
    if (close !== -1 && chars[close + 1] === "]") {
      const name = chars.slice(at + 2, close).join("");
      const next = close + 2;
      if (kind === ":") {
        const cls = CLASSES[name];
        if (name === "ascii") return { next, test: (c) => (c.codePointAt(0) ?? 128) < 128 };
        return cls === undefined ? { next, test: () => false } : { next, test: (c) => cls.test(c) };
      }
      return { next, test: (c) => c === name, char: name };
    }
  }
  if (open === "\\" && at + 1 < chars.length) {
    const char = chars[at + 1] ?? "";
    return { next: at + 2, test: (c) => c === char, char };
  }
  if (open === undefined) return undefined;
  return { next: at + 1, test: (c) => c === open, char: open };
}

/** A bracket expression starting at `[`, or undefined when it never closes. */
function parseBracket(
  chars: readonly string[],
  start: number,
): { next: number; bracket: Bracket } | undefined {
  let at = start + 1;
  const negated = chars[at] === "!" || chars[at] === "^";
  if (negated) at += 1;
  const tests: ((c: string) => boolean)[] = [];
  let first = true;
  while (at < chars.length) {
    if (chars[at] === "]" && !first) {
      const bracket = { negated, test: (c: string) => tests.some((t) => t(c)) };
      return { next: at + 1, bracket };
    }
    first = false;
    const member = bracketMember(chars, at);
    if (member === undefined) return undefined;
    const low = member.char;
    if (low !== undefined && chars[member.next] === "-" && chars[member.next + 1] !== "]") {
      const high = bracketMember(chars, member.next + 1);
      if (high?.char !== undefined) {
        const lo = low.codePointAt(0) ?? 0;
        const hi = high.char.codePointAt(0) ?? 0;
        tests.push((c) => {
          const code = c.codePointAt(0) ?? -1;
          return code >= lo && code <= hi;
        });
        at = high.next;
        continue;
      }
    }
    tests.push(member.test);
    at = member.next;
  }
  return undefined;
}

/** The index of the `)` closing an extglob opened at `start`, skipping escapes and brackets. */
function extglobEnd(
  chars: readonly string[],
  start: number,
): { end: number; bars: number[] } | undefined {
  let depth = 0;
  const bars: number[] = [];
  let at = start;
  while (at < chars.length) {
    const c = chars[at];
    if (c === "\\") {
      at += 2;
      continue;
    }
    if (c === "[") {
      const bracket = parseBracket(chars, at);
      if (bracket !== undefined) {
        at = bracket.next;
        continue;
      }
    }
    if (c === "(") depth += 1;
    if (c === ")") {
      if (depth === 0) return { end: at, bars };
      depth -= 1;
    }
    if (c === "|" && depth === 0) bars.push(at);
    at += 1;
  }
  return undefined;
}

const EXT_OPS = new Set(["?", "*", "+", "@", "!"]);

function isExtOp(c: string | undefined): c is ExtOp {
  return c !== undefined && EXT_OPS.has(c);
}

function parse(chars: readonly string[], from: number, to: number): Node[] {
  const nodes: Node[] = [];
  let at = from;
  while (at < to) {
    const c = chars[at] ?? "";
    if (isExtOp(c) && chars[at + 1] === "(") {
      const close = extglobEnd(chars, at + 2);
      if (close !== undefined && close.end < to) {
        const bounds = [at + 1, ...close.bars, close.end];
        const alternatives = bounds
          .slice(1)
          .map((end, i) => parse(chars, (bounds[i] ?? 0) + 1, end));
        nodes.push({ kind: "ext", op: c, alternatives });
        at = close.end + 1;
        continue;
      }
    }
    if (c === "*") {
      nodes.push({ kind: "star" });
    } else if (c === "?") {
      nodes.push({ kind: "any" });
    } else if (c === "[") {
      const bracket = parseBracket(chars, at);
      if (bracket !== undefined && bracket.next <= to) {
        nodes.push({ kind: "bracket", bracket: bracket.bracket });
        at = bracket.next;
        continue;
      }
      nodes.push({ kind: "literal", char: c });
    } else if (c === "\\" && at + 1 < to) {
      nodes.push({ kind: "literal", char: chars[at + 1] ?? "" });
      at += 2;
      continue;
    } else {
      nodes.push({ kind: "literal", char: c });
    }
    at += 1;
  }
  return nodes;
}

type Span = { readonly text: readonly string[]; readonly to: number };

function anyAlt(alternatives: readonly Node[][], span: Span, start: number, end: number): boolean {
  return alternatives.some((alt) => matchSeq(alt, span.text, start, end));
}

/** `op(alternatives)` over `text[pos..end)` for some `end`, then `rest(end)`. */
function matchExt(
  node: Extract<Node, { kind: "ext" }>,
  span: Span,
  pos: number,
  rest: (end: number) => boolean,
): boolean {
  const { op, alternatives } = node;
  if (op === "!") {
    for (let end = pos; end <= span.to; end += 1) {
      if (!anyAlt(alternatives, span, pos, end) && rest(end)) return true;
    }
    return false;
  }
  if ((op === "?" || op === "*") && rest(pos)) return true;
  for (let end = pos + (op === "@" || op === "?" ? 0 : 1); end <= span.to; end += 1) {
    if (!anyAlt(alternatives, span, pos, end)) continue;
    if (rest(end)) return true;
    const again = (op === "*" || op === "+") && end > pos;
    if (again && matchExt({ ...node, op: "*" }, span, end, rest)) return true;
  }
  return false;
}

/** Does `nodes` match `text[from..to)` exactly? */
function matchSeq(
  nodes: readonly Node[],
  text: readonly string[],
  from: number,
  to: number,
): boolean {
  const span: Span = { text, to };
  const memo = new Map<number, boolean>();
  const step = (i: number, pos: number): boolean => {
    const key = i * (to + 1) + pos;
    const cached = memo.get(key);
    if (cached !== undefined) return cached;
    const result = stepAt(i, pos);
    memo.set(key, result);
    return result;
  };
  const stepAt = (i: number, pos: number): boolean => {
    const node = nodes[i];
    if (node === undefined) return pos === to;
    const char = text[pos];
    switch (node.kind) {
      case "literal":
        return char === node.char && step(i + 1, pos + 1);
      case "any":
        return pos < to && step(i + 1, pos + 1);
      case "bracket":
        return (
          char !== undefined &&
          pos < to &&
          node.bracket.test(char) !== node.bracket.negated &&
          step(i + 1, pos + 1)
        );
      case "star":
        for (let end = pos; end <= to; end += 1) if (step(i + 1, end)) return true;
        return false;
      case "ext":
        return matchExt(node, span, pos, (end) => step(i + 1, end));
      default: {
        const never: never = node;
        return never;
      }
    }
  };
  return step(0, from);
}

export type BashPattern = (text: string) => boolean;

/** Compile `pattern` once; the result tests whole strings, as `[[ == ]]` does. */
export function compileBashPattern(pattern: string): BashPattern {
  const chars = Array.from(pattern);
  const nodes = parse(chars, 0, chars.length);
  return (text) => {
    const units = Array.from(text);
    return matchSeq(nodes, units, 0, units.length);
  };
}

/** `[[ text == $pattern ]]` under `shopt -s extglob`. */
export function bashPatternMatch(pattern: string, text: string): boolean {
  return compileBashPattern(pattern)(text);
}
