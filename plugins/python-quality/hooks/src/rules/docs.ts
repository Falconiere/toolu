/**
 * The docs advisory (#266, 90-docs): a column-0, non-underscore `def` or
 * `class` should open with a docstring. The signature is followed to the line
 * ending in `:`, then the first non-blank line must open a `"""` or `'''`
 * string (one `r`/`R`/`f`/`F` prefix allowed). A line consumed as that body
 * is not itself checked for a definition, as in the awk program.
 */
import { head, type PyFile } from "./py-file.ts";

const DEFINITION = /^(async[ \t]+def|def|class)[ \t]+[A-Za-z_][A-Za-z0-9_]*/;

function isBlank(line: string): boolean {
  return line.replace(/^[ \t]+|[ \t]+$/g, "") === "";
}

function endsWithColon(line: string): boolean {
  const code = line.replace(/[ \t]#.*$/s, "").replace(/[ \t]+$/, "");
  return code.endsWith(":");
}

function opensDocstring(line: string): boolean {
  const text = line.replace(/^[ \t]+/, "");
  const at = /^[rRfF]/.test(text) ? 1 : 0;
  const quote = text.slice(at, at + 3);
  return quote === '"""' || quote === "'''";
}

function definitionName(line: string): string {
  const rest = line.replace(/^(async[ \t]+def|def|class)[ \t]+/, "");
  return /^[A-Za-z_][A-Za-z0-9_]*/.exec(rest)?.[0] ?? "";
}

/** The advisory text, or "" when every public top-level definition has a docstring. */
export function docs(f: PyFile): string {
  const missing: string[] = [];
  let state: "scan" | "signature" | "body" = "scan";
  let at = { line: 0, name: "" };
  for (const [index, line] of f.lines.entries()) {
    if (state === "body") {
      if (isBlank(line)) continue;
      if (!opensDocstring(line)) missing.push(`${String(at.line)}: ${at.name}`);
      state = "scan";
      continue;
    }
    if (state === "signature") {
      if (endsWithColon(line)) state = "body";
      continue;
    }
    if (!DEFINITION.test(line)) continue;
    const name = definitionName(line);
    if (name.startsWith("_")) continue;
    at = { line: index + 1, name };
    state = endsWithColon(line) ? "body" : "signature";
  }
  if (missing.length === 0) return "";
  return `Public def/class missing a docstring in ${f.file.path} — add a concise one:\n${head(missing, 3)}`;
}
