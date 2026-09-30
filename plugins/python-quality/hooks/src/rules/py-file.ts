/**
 * The edited Python file as every rule reads it (#266), and the grep/awk
 * idioms the bash fragments were built from. awk splits records on `\n`
 * only, so a CRLF line keeps its `\r`; `[ \t]` in the awk programs does not
 * match it, POSIX `[[:space:]]` (`SP`) does.
 */
import type { EditedFile } from "@toolu/core/quality";
import type { RegistryContext } from "@toolu/core/registry";

/** `[[:space:]]` in the C/UTF-8 locales the hooks run under. */
export const SP = "[ \\t\\n\\v\\f\\r]";

/** A POSIX ERE (with `SP` for `[[:space:]]`) as a JavaScript regex; `.` also matches `\r`. */
export function ere(source: string): RegExp {
  return new RegExp(source.replaceAll("[[:space:]]", SP), "s");
}

/** The limits the rules check against, resolved once per event. */
export type PyLimits = {
  readonly fileLines: number;
  readonly fnLines: number;
  readonly noMocks: boolean;
};

export type PyFile = {
  readonly file: EditedFile;
  /** The file's lines as awk reads them: no trailing empty line for a final newline. */
  readonly lines: readonly string[];
  readonly ctx: RegistryContext;
  readonly limits: PyLimits;
};

/** `grep` / `awk` lines of `text`. */
export function splitLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return lines;
}

/** `head -N`, joined as `$(...)` returns it: newline-separated, no trailing newline. */
export function head(items: readonly string[], n: number): string {
  return items.slice(0, n).join("\n");
}

/** `case "$base" in test_*.py|*_test.py)`: the colocated test naming convention. */
export function isPythonTestName(base: string): boolean {
  return (base.startsWith("test_") && base.endsWith(".py")) || base.endsWith("_test.py");
}
