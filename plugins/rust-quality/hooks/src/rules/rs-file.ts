/**
 * The edited Rust file as every rule reads it (#267), and the grep/awk idioms
 * the bash fragments were built from. Regexes use the `s` flag so `.` also
 * matches `\r`, as it does in POSIX tools, and `SP` for `[[:space:]]`.
 */
import { basename } from "node:path";
import type { EditedFile } from "@toolu/core/quality";
import type { RegistryContext } from "@toolu/core/registry";

/** `[[:space:]]` in the C/UTF-8 locales the hooks run under. */
const SP = "[ \\t\\n\\v\\f\\r]";

/** A POSIX ERE (with `[[:space:]]`) as a JavaScript regex. */
export function ere(source: string): RegExp {
  return new RegExp(source.replaceAll("[[:space:]]", SP), "s");
}

/** The limits and switches the rules check against, resolved once per event. */
export type RsLimits = {
  readonly fileLines: number;
  readonly fnLines: number;
  readonly implLines: number;
  readonly noMocks: boolean;
  /** `rust-unsafe-exemptions.txt`: crates whose paths may hold `unsafe`. */
  readonly unsafeExemptions: readonly string[];
};

export type RsFile = {
  readonly file: EditedFile;
  /** The file's lines as awk reads them: no trailing empty line for a final newline. */
  readonly lines: readonly string[];
  readonly ctx: RegistryContext;
  readonly limits: RsLimits;
};

/** awk records of `text`. */
export function splitLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return lines;
}

/** bash `[[ "$FILE_PATH" == *<part>* ]]`. */
export function pathHas(f: RsFile, part: string): boolean {
  return f.file.path.includes(part);
}

/** `case "$(basename "$FILE_PATH")" in *_test.rs|*_tests.rs)`. */
export function testFileName(path: string): boolean {
  return /_tests?\.rs$/s.test(basename(path));
}
