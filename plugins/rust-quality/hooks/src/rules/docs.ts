/**
 * The docs advisory (#267), the port of 90-docs: public items in non-test src/
 * files with no `///` doc, and doc blocks over 12 lines. It never touches the
 * gate.
 */
import { ere, pathHas, type RsFile } from "./rs-file.ts";

const BLANK = ere(String.raw`^[[:space:]]*$`);
const ATTRIBUTE = ere(String.raw`^[[:space:]]*#\[`);
const PUBLIC_ITEM = ere(
  String.raw`^[[:space:]]*pub(\([^)]+\))?[[:space:]]+(fn|struct|enum|trait|const|static|type|mod)[[:space:]]`,
);
const DOC_LINE = ere(String.raw`^[[:space:]]*(///|//!)`);
const DOC_RUN_LINE = ere(String.raw`^[[:space:]]*//[/!]`);
const MAX_DOC_RUN = 12;

/** `N: line` for public items whose previous non-blank, non-attribute line is no doc, at most 3. */
function undocumented(lines: readonly string[]): string[] {
  const found: string[] = [];
  let prev = "";
  lines.forEach((line, i) => {
    if (BLANK.test(line) || ATTRIBUTE.test(line)) return;
    if (PUBLIC_ITEM.test(line) && !DOC_LINE.test(prev)) found.push(`${String(i + 1)}: ${line}`);
    prev = line;
  });
  return found.slice(0, 3);
}

/** Doc runs over the cap, at most 2. */
function verbose(lines: readonly string[]): string[] {
  const found: string[] = [];
  let run = 0;
  let start = 0;
  const close = () => {
    if (run > MAX_DOC_RUN)
      found.push(`${String(start)}: doc block is ${String(run)} lines — trim to the essentials`);
    run = 0;
  };
  lines.forEach((line, i) => {
    if (DOC_RUN_LINE.test(line)) {
      if (run === 0) start = i + 1;
      run += 1;
    } else {
      close();
    }
  });
  close();
  return found.slice(0, 2);
}

/** 90-docs, the missing-doc and verbose-doc advisories. */
export function docs(f: RsFile, isTest: boolean): string[] {
  if (!pathHas(f, "/src/") || isTest) return [];
  const path = f.file.path;
  const missing = undocumented(f.lines);
  const long = verbose(f.lines);
  return [
    missing.length === 0
      ? ""
      : `Public items missing a /// doc comment in ${path} — add a concise one-line doc:\n${missing.join("\n")}`,
    long.length === 0
      ? ""
      : `Verbose doc comment in ${path} — docs must be present but concise:\n${long.join("\n")}`,
  ];
}
