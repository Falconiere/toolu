/**
 * File, function and impl size (#267), ports of 10-size-file, 50-size-fn and
 * 55-size-impl. The fn and impl counters are the awk brace-depth scan line for
 * line: single-line string contents and `'{'`/`'}'` literals are stripped, the
 * range starts at the head and ends where the depth returns to zero.
 */
import { countCodeLines, detectClippy, hasUnterminatedBlock } from "@toolu/core/detect";
import { ere, type RsFile } from "./rs-file.ts";

/** 10-size-file. */
export function fileTooLong(f: RsFile): string | undefined {
  const max = f.limits.fileLines;
  const count = countCodeLines(f.file.absolute) ?? 0;
  if (count <= max) return undefined;
  const where = { env: f.ctx.env, ...(f.ctx.cwd === undefined ? {} : { cwd: f.ctx.cwd }) };
  const hint = detectClippy(where)
    ? "split into submodules (clippy enforces complexity here)"
    : "split into submodules";
  const approx = hasUnterminatedBlock(f.file.absolute)
    ? " (size approximated — an unterminated /* or a string containing /* may be affecting the count)"
    : "";
  return `File exceeds ${String(max)}-line limit: ${f.file.path} (${String(count)} code lines, blanks/comments excluded)${approx} — ${hint}`;
}

/** awk: escaped quotes, then single-line "…" contents, then brace char literals. */
function strip(line: string): string {
  return line
    .replaceAll('\\"', "")
    .replaceAll(/"[^"]*"/g, "")
    .replaceAll(/'[{}]'/g, "");
}

function count(text: string, char: string): number {
  return text.split(char).length - 1;
}

const FN_HEAD = ere(
  String.raw`^[[:space:]]*(pub(\([^)]+\))?[[:space:]]+)?((async|const|unsafe|extern)([[:space:]]+"[^"]*")?[[:space:]]+)*fn `,
);
const IMPL_HEAD = ere(String.raw`^[[:space:]]*(unsafe[[:space:]]+)?impl[ \t\n\v\f\r<]`);
const ENDS_STATEMENT = ere(String.raw`;[[:space:]]*$`);

/**
 * Whether any range opened by `head` runs past `max` lines. With
 * `releaseOnSemicolon`, a head whose line ends in `;` before any `{` is a
 * body-less declaration.
 */
function anyLongRange(
  lines: readonly string[],
  head: RegExp,
  max: number,
  releaseOnSemicolon: boolean,
): boolean {
  let open: { start: number; depth: number; opened: boolean } | undefined;
  let long = false;
  lines.forEach((line, i) => {
    if (open === undefined && head.test(line)) open = { start: i, depth: 0, opened: false };
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
    if (releaseOnSemicolon && !open.opened && ENDS_STATEMENT.test(line)) open = undefined;
  });
  return long;
}

/** 50-size-fn; test files are exempt. */
export function functionTooLong(f: RsFile, isTest: boolean): string | undefined {
  const max = f.limits.fnLines;
  if (isTest || !anyLongRange(f.lines, FN_HEAD, max, true)) return undefined;
  return `Function too long in ${f.file.path} (>${String(max)} lines) — extract helpers.`;
}

/** 55-size-impl; test files are exempt. */
export function implTooLong(f: RsFile, isTest: boolean): string | undefined {
  const max = f.limits.implLines;
  if (isTest || !anyLongRange(f.lines, IMPL_HEAD, max, false)) return undefined;
  return `Impl block too large in ${f.file.path} (>${String(max)} lines) — split into trait impls or modules.`;
}
