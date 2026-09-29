/**
 * File and function size (#265), ports of 25-size-file and 30-size-fn. The
 * function counter is the awk brace-depth scan line for line: it strips
 * single-line string contents, starts at a `function`, a `const NAME = (…`
 * or `= function`, or a method head, and ends where the depth returns to zero.
 */
import { countCodeLines, hasUnterminatedBlock, tsLinter } from "@toolu/core/detect";
import { ere, type TsFile } from "./ts-file.ts";

/** 25-size-file. */
export function fileTooLong(f: TsFile): string | undefined {
  const max = f.limits.fileLines;
  const count = countCodeLines(f.file.absolute) ?? 0;
  if (count <= max) return undefined;
  let hint = "split into smaller modules";
  const linter = tsLinter({
    env: f.ctx.env,
    ...(f.ctx.cwd === undefined ? {} : { cwd: f.ctx.cwd }),
  });
  if (linter !== undefined && f.limits.fileSource === "native") {
    hint += ` (${linter} enforces this max-lines limit)`;
  } else if (linter === "biome" && f.limits.fileSource === "default") {
    hint += ` (biome has no max-lines equivalent — gate uses the ${String(max)}-line default)`;
  } else if (linter !== undefined && f.limits.fileSource === "default") {
    hint += ` (${linter} is present but the gate's limit didn't come from its config (unparsed config form or a per-glob override) — gate uses the ${String(max)}-line default; align them)`;
  }
  const approx = hasUnterminatedBlock(f.file.absolute)
    ? " (size approximated — an unterminated /* or a string containing /* may be affecting the count)"
    : "";
  return `TS file exceeds ${String(max)}-line limit: ${f.file.path} (${String(count)} code lines, blanks/comments excluded)${approx} — ${hint}`;
}

/** awk `strip`: escaped quotes, then "…", '…' and `…` contents on one line. */
function strip(line: string): string {
  return line
    .replaceAll('\\"', "")
    .replaceAll(/"[^"]*"/g, "")
    .replaceAll(/'[^']*'/g, "")
    .replaceAll(/`[^`]*`/g, "");
}

const FUNCTION_DECL = ere(
  String.raw`^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?(async[[:space:]]+)?function[ \t*]`,
);
const CONST_FN = ere(
  String.raw`^[[:space:]]*(export[[:space:]]+)?(default[[:space:]]+)?const[[:space:]]+[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*=[[:space:]]*(async[[:space:]]+)?(\(|function[ \t(*])`,
);
const METHOD_HEAD = ere(
  String.raw`^[[:space:]]+(public[[:space:]]+|private[[:space:]]+|protected[[:space:]]+|static[[:space:]]+|async[[:space:]]+|override[[:space:]]+|readonly[[:space:]]+|get[[:space:]]+|set[[:space:]]+|\*[[:space:]]*)*[A-Za-z_$][A-Za-z0-9_$]*[[:space:]]*(<[^(){}]*>)?[[:space:]]*\(`,
);
const KEYWORD_HEAD = ere(
  String.raw`^[[:space:]]*(if|for|while|switch|catch|return|do|else|function|await|with|yield|throw|new|typeof|delete|void|in|of|case)[^A-Za-z0-9_$]`,
);
const METHOD_BODY = ere(String.raw`\)([[:space:]]*:[^={]*)?[[:space:]]*\{[[:space:]]*$`);
const ENDS_STATEMENT = ere(String.raw`;[[:space:]]*$`);

function startsFunction(line: string): boolean {
  const s = strip(line);
  if (FUNCTION_DECL.test(s) || CONST_FN.test(s)) return true;
  return (
    METHOD_HEAD.test(s) &&
    !KEYWORD_HEAD.test(s) &&
    !s.includes("=>") &&
    !ENDS_STATEMENT.test(s) &&
    METHOD_BODY.test(s)
  );
}

function count(text: string, char: string): number {
  return text.split(char).length - 1;
}

/** Whether any function runs past `max` lines, measured as the awk counter did. */
function anyLongFunction(lines: readonly string[], max: number): boolean {
  let open: { start: number; depth: number; opened: boolean } | undefined;
  let long = false;
  lines.forEach((line, i) => {
    if (open === undefined && startsFunction(line)) open = { start: i, depth: 0, opened: false };
    if (open === undefined) return;
    const s = strip(line);
    const opens = count(s, "{");
    open.depth += opens - count(s, "}");
    if (opens > 0) open.opened = true;
    if (open.opened && open.depth <= 0) {
      if (i - open.start > max) long = true;
      open = undefined;
      return;
    }
    // A brace-less one-line form (`const sq = (x) => x * x;`) ends on its `;`.
    if (!open.opened && ENDS_STATEMENT.test(line)) open = undefined;
  });
  return long;
}

/** 30-size-fn. */
export function functionTooLong(f: TsFile): string | undefined {
  if (!anyLongFunction(f.lines, f.limits.fnLines)) return undefined;
  return `Function too long in ${f.file.path} (>${String(f.limits.fnLines)} lines) — simplify or split`;
}
