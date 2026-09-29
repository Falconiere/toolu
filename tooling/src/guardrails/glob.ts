/**
 * Shell `case` pattern matching, the semantics every guardrails glob had in
 * bash: `*` matches any run of characters INCLUDING `/`, `?` one character,
 * `[...]` / `[!...]` a bracket expression, `\x` a literal. Patterns come from
 * config (testGlob, barrelExempt, fileSize overrides, filenameCase,
 * secrets.scanExempt), so an unterminated bracket is read literally, as bash does.
 */

const POSIX_CLASSES: Record<string, string> = {
  alnum: "\\p{L}\\p{Nd}",
  alpha: "\\p{L}",
  digit: "0-9",
  lower: "\\p{Ll}",
  upper: "\\p{Lu}",
  space: " \\t\\n\\v\\f\\r",
  blank: " \\t",
  punct: "!-\\/:-@\\[-`{-~",
  xdigit: "0-9A-Fa-f",
  cntrl: "\\x00-\\x1f\\x7f",
};

/** Regex syntax characters: the only identity escapes a `u`-mode RegExp accepts. */
const SYNTAX = /[\\^$.*+?()[\]{}|/]/g;

function escapeChar(char: string): string {
  return char.replace(SYNTAX, "\\$&");
}

/** Inside a class `-` is syntax too (u-mode accepts `\-` there). */
function escapeInClass(char: string): string {
  return char === "-" ? "\\-" : escapeChar(char);
}

/** Translate a bracket body starting after `[`; returns [regex, next index] or null. */
function bracket(pattern: string, start: number): [string, number] | null {
  let i = start;
  let negate = false;
  if (pattern[i] === "!" || pattern[i] === "^") {
    negate = true;
    i += 1;
  }
  let body = "";
  let first = true;
  while (i < pattern.length) {
    const char = pattern.charAt(i);
    if (char === "]" && !first) return [`[${negate ? "^" : ""}${body}]`, i + 1];
    first = false;
    if (char === "[" && pattern[i + 1] === ":") {
      const close = pattern.indexOf(":]", i + 2);
      const name = close === -1 ? "" : pattern.slice(i + 2, close);
      const cls = POSIX_CLASSES[name];
      if (cls !== undefined) {
        body += cls;
        i = close + 2;
        continue;
      }
    }
    if (char === "\\" && i + 1 < pattern.length) {
      body += escapeInClass(pattern.charAt(i + 1));
      i += 2;
      continue;
    }
    body += char === "-" ? "-" : escapeInClass(char);
    i += 1;
  }
  return null;
}

/** Compile a shell pattern to an anchored RegExp. */
export function globToRegExp(pattern: string): RegExp {
  let out = "";
  let i = 0;
  while (i < pattern.length) {
    const char = pattern.charAt(i);
    if (char === "*") {
      out += "[\\s\\S]*";
      i += 1;
    } else if (char === "?") {
      out += "[\\s\\S]";
      i += 1;
    } else if (char === "[") {
      const parsed = bracket(pattern, i + 1);
      if (parsed === null) {
        out += "\\[";
        i += 1;
      } else {
        out += parsed[0];
        i = parsed[1];
      }
    } else if (char === "\\" && i + 1 < pattern.length) {
      out += escapeChar(pattern.charAt(i + 1));
      i += 2;
    } else {
      out += escapeChar(char);
      i += 1;
    }
  }
  return new RegExp(`^${out}$`, "u");
}

export function matchGlob(pattern: string, subject: string): boolean {
  return globToRegExp(pattern).test(subject);
}

/** True when any pattern in the list matches. */
export function matchAny(patterns: readonly string[], subject: string): boolean {
  return patterns.some((pattern) => matchGlob(pattern, subject));
}

/**
 * A POSIX ERE from config (filenameCase `regex`, read by awk in bash) as a JS
 * RegExp: bracket classes such as `[[:lower:]]` are expanded, the rest of ERE
 * syntax is already JavaScript syntax.
 */
export function ereToRegExp(ere: string): RegExp {
  const translated = ere.replace(
    /\[:([a-z]+):\]/g,
    (whole: string, name: string) => POSIX_CLASSES[name] ?? whole,
  );
  const unicode = translated !== ere;
  // Only a substituted class needs `u` (for \p{…}); without one, keep the
  // permissive non-unicode syntax so any ERE escape awk accepted still compiles.
  return new RegExp(translated, unicode ? "u" : "");
}
