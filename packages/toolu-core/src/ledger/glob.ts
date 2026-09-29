/**
 * Bash `case "$path" in $glob)` pattern matching (#256), as used by the
 * docs-sync globs. `*` matches any run of characters (slashes included), `?`
 * matches one character, `[...]` is a bracket expression (`!` or `^` negates,
 * ranges and `[:class:]` names work), and `\` quotes the next character.
 * extglob is off, as in a bash script.
 */

const CLASSES: Record<string, string> = {
  alnum: "\\p{L}\\p{N}",
  alpha: "\\p{L}",
  blank: " \\t",
  cntrl: "\\p{Cc}",
  digit: "0-9",
  graph: "\\p{L}\\p{N}\\p{P}\\p{S}",
  lower: "\\p{Ll}",
  print: "\\p{L}\\p{N}\\p{P}\\p{S} ",
  punct: "\\p{P}\\p{S}",
  space: "\\s",
  upper: "\\p{Lu}",
  xdigit: "0-9A-Fa-f",
};

/** A literal outside a bracket expression. */
const escapeChar = (c: string): string => c.replace(/[\\^$.*+?()[\]{}|/]/g, "\\$&");

/** A literal inside a bracket expression. */
const escapeClassChar = (c: string): string => c.replace(/[\\^[\]-]/g, "\\$&");

/** The bracket expression starting at `pattern[start] === "["`: its regex and end index, or undefined when unclosed. */
function bracket(chars: string[], start: number): { source: string; end: number } | undefined {
  let i = start + 1;
  let negate = false;
  if (chars[i] === "!" || chars[i] === "^") {
    negate = true;
    i += 1;
  }
  let body = "";
  let first = true;
  while (i < chars.length) {
    const c = chars[i] ?? "";
    if (c === "]" && !first) return { source: `[${negate ? "^" : ""}${body}]`, end: i };
    first = false;
    if (c === "[" && chars[i + 1] === ":") {
      const close = chars.indexOf(":", i + 2);
      const name = close === -1 ? "" : chars.slice(i + 2, close).join("");
      if (close !== -1 && chars[close + 1] === "]" && CLASSES[name] !== undefined) {
        body += CLASSES[name];
        i = close + 2;
        continue;
      }
    }
    if (c === "\\" && i + 1 < chars.length) {
      body += escapeClassChar(chars[i + 1] ?? "");
      i += 2;
      continue;
    }
    if (c === "-" && body !== "" && chars[i + 1] !== "]" && i + 1 < chars.length) {
      body += "-";
      i += 1;
      continue;
    }
    body += escapeClassChar(c);
    i += 1;
  }
  return undefined;
}

/** Compile one bash glob to an anchored RegExp. */
export function globToRegExp(pattern: string): RegExp {
  const chars = Array.from(pattern);
  let source = "";
  for (let i = 0; i < chars.length; i += 1) {
    const c = chars[i] ?? "";
    if (c === "*") source += "[\\s\\S]*";
    else if (c === "?") source += "[\\s\\S]";
    else if (c === "\\" && i + 1 < chars.length) {
      source += escapeChar(chars[i + 1] ?? "");
      i += 1;
    } else if (c === "[") {
      const expr = bracket(chars, i);
      if (expr === undefined) source += "\\[";
      else {
        source += expr.source;
        i = expr.end;
      }
    } else source += escapeChar(c);
  }
  return new RegExp(`^${source}$`, "u");
}

/** `_vd_matches_any PATH GLOBS`: does `path` match any non-empty glob? */
export function matchesAny(path: string, globs: readonly string[]): boolean {
  return globs.some((glob) => glob !== "" && globToRegExp(glob).test(path));
}
