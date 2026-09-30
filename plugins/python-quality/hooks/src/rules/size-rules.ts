/**
 * The size rules (#266): file length in code lines (10-size-file) and the
 * indent-based function span (50-size-fn), ported from their awk programs.
 */
import { countPythonCodeLines } from "@toolu/core/detect";
import type { PyFile } from "./py-file.ts";

/** 10-size-file: non-blank, non-`#` lines over `lang.python.maxFileLines`. */
export function fileTooLong(f: PyFile): string | undefined {
  const max = f.limits.fileLines;
  const count = countPythonCodeLines(f.file.absolute) ?? 0;
  if (count <= max) return undefined;
  return `File exceeds ${String(max)}-line limit: ${f.file.path} (${String(count)} code lines, blanks/comments excluded) — split into submodules. Override via lang.python.maxFileLines.`;
}

/** awk `lead`: leading spaces and tabs. */
function lead(line: string): number {
  let n = 0;
  while (n < line.length && (line[n] === " " || line[n] === "\t")) n += 1;
  return n;
}

const DEF = /^[ \t]*(async[ \t]+)?def[ \t]/;
const NAME = /def[ \t]+[A-Za-z_][A-Za-z0-9_]*/;

type Span = { name: string; start: number; indent: number; count: number };

function spanReport(span: Span): string {
  return `${span.name}:${String(span.start)} (${String(span.count)} lines)`;
}

/**
 * 50-size-fn: a `def` opens a span at its own indent, closed by the next
 * non-blank line indented no deeper. Only code lines count, the def line
 * included; a def nested in an open span belongs to it.
 */
export function functionTooLong(f: PyFile): string | undefined {
  const max = f.limits.fnLines;
  const long: string[] = [];
  let open: Span | undefined;
  f.lines.forEach((raw, index) => {
    const trimmed = raw.replace(/^[ \t]+/, "");
    const blank = trimmed === "";
    if (open !== undefined && !blank && lead(raw) <= open.indent) {
      if (open.count > max) long.push(spanReport(open));
      open = undefined;
    }
    if (open === undefined && DEF.test(raw)) {
      const name = NAME.exec(trimmed)?.[0].replace(/^def[ \t]+/, "") ?? "";
      open = { name, start: index + 1, indent: lead(raw), count: 0 };
    }
    if (open !== undefined && !blank && !trimmed.startsWith("#")) open.count += 1;
  });
  if (open !== undefined && open.count > max) long.push(spanReport(open));
  if (long.length === 0) return undefined;
  return `Function too long in ${f.file.path} (>${String(max)} lines) — extract helpers.\n${long.join("\n")}`;
}
