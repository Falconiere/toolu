/**
 * The edited TypeScript file as every rule reads it (#265), and the grep/awk
 * idioms the bash fragments were built from: lines as grep sees them,
 * `grep -n` excerpts, `head -N`. Regexes use the `s` flag so `.` also
 * matches `\r`, as it does in POSIX tools, and `SP` for `[[:space:]]`.
 */
import type { EditedFile } from "@toolu/core/quality";
import type { RegistryContext } from "@toolu/core/registry";

/** `[[:space:]]` in the C/UTF-8 locales the hooks run under. */
export const SP = "[ \\t\\n\\v\\f\\r]";

/** A POSIX ERE (with `SP` for `[[:space:]]`) as a JavaScript regex. */
export function ere(source: string): RegExp {
  return new RegExp(source.replaceAll("[[:space:]]", SP), "s");
}

/** The limits the size rules check against, resolved once per event. */
export type TsLimits = {
  readonly fileLines: number;
  readonly fileSource: "override" | "native" | "default";
  readonly fnLines: number;
  readonly noMocks: boolean;
};

export type TsFile = {
  readonly file: EditedFile;
  /** The file's lines as grep reads them: no trailing empty line for a final newline. */
  readonly lines: readonly string[];
  readonly ctx: RegistryContext;
  readonly limits: TsLimits;
  /** The project's package manager, as `detect_node_pm` named it. */
  readonly pm: string;
};

/** `grep` / `awk` lines of `text`. */
export function splitLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return lines;
}

/** `grep -n PATTERN`: `N:line` for every matching line. */
export function numbered(lines: readonly string[], pattern: RegExp): string[] {
  return lines.flatMap((line, i) => (pattern.test(line) ? [`${String(i + 1)}:${line}`] : []));
}

/** `grep -c PATTERN`. */
export function countMatches(lines: readonly string[], pattern: RegExp): number {
  return lines.filter((line) => pattern.test(line)).length;
}

/** `head -N`, joined as `$(...)` returns it: newline-separated, no trailing newline. */
export function head(items: readonly string[], n: number): string {
  return items.slice(0, n).join("\n");
}

/** Over `grep -n` output, drop whole-line `//` comments (the fragments' shared `grep -vE`). */
export function withoutCommentLines(rows: readonly string[]): string[] {
  const comment = ere("^[0-9]+:[[:space:]]*//");
  return rows.filter((row) => !comment.test(row));
}

/** `add_error "<header>"$'\n'"<excerpt>"`, or undefined when the excerpt is empty. */
export function withExcerpt(header: string, excerpt: string): string | undefined {
  return excerpt === "" ? undefined : `${header}\n${excerpt}`;
}

/** bash `[[ "$FILE_PATH" == *<part>* ]]`. */
export function pathHas(f: TsFile, part: string): boolean {
  return f.file.path.includes(part);
}

/** `\.(test|spec)\.(ts|tsx)$` on the path. */
export function isTestPath(path: string): boolean {
  return /\.(test|spec)\.(ts|tsx)$/s.test(path);
}

/** The hook environment as a child process's: unset keys dropped. */
export function spawnEnv(f: TsFile): Record<string, string> {
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(f.ctx.env)) if (value !== undefined) env[key] = value;
  return env;
}
