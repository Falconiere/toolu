/**
 * Code-line counting (#254): ports of `count_code_lines`,
 * `count_python_code_lines` and `has_unterminated_block`, reading the file in
 * bounded chunks (`eachLine`) instead of shelling out to awk, grep and wc.
 * `undefined` means the file could not be read, where awk printed nothing.
 */
import { eachLine, isRegularFile } from "./detect-read.ts";

/** awk `gsub(/^[ \t]+|[ \t]+$/, "")`: only spaces and tabs, so a `\r` is content. */
function trimBlanks(line: string): string {
  return line.replace(/^[ \t]+|[ \t]+$/g, "");
}

/**
 * Strip `/* … *\/` blocks from `line`, given whether a block was already open.
 * Returns the code left and whether a block is still open at the end.
 */
function stripBlocks(
  line: string,
  inBlock: boolean,
): { code: string; inBlock: boolean } | undefined {
  let rest = line;
  if (inBlock) {
    const close = rest.indexOf("*/");
    if (close === -1) return undefined;
    rest = rest.slice(close + 2);
  }
  for (;;) {
    const open = rest.indexOf("/*");
    if (open === -1) return { code: rest, inBlock: false };
    const after = rest.slice(open + 2);
    const close = after.indexOf("*/");
    if (close === -1) return { code: rest.slice(0, open), inBlock: true };
    rest = rest.slice(0, open) + after.slice(close + 2);
  }
}

/**
 * `count_code_lines FILE`: lines of code once blank lines, `//` comments and
 * `/* *\/` blocks are removed (TypeScript and Rust). String literals are not
 * tracked, so a `"/*"` can open a block that never closes; the count then
 * falls back to the raw line count, failing toward flagging a large file.
 */
export function countCodeLines(path: string): number | undefined {
  const count = { inBlock: false, records: 0, code: 0 };
  const walk = eachLine(path, (line) => {
    count.records += 1;
    const stripped = stripBlocks(line, count.inBlock);
    if (stripped === undefined) return;
    count.inBlock = stripped.inBlock;
    const comment = stripped.code.indexOf("//");
    const kept = comment === -1 ? stripped.code : stripped.code.slice(0, comment);
    if (trimBlanks(kept) !== "") count.code += 1;
  });
  if (walk === "unreadable") return undefined;
  return count.inBlock ? count.records : count.code;
}

/**
 * `count_python_code_lines FILE`: non-blank lines that do not start with `#`.
 * Trailing comments and docstring lines count, failing toward flagging.
 */
export function countPythonCodeLines(path: string): number | undefined {
  let code = 0;
  const walk = eachLine(path, (line) => {
    const trimmed = trimBlanks(line);
    if (trimmed !== "" && !trimmed.startsWith("#")) code += 1;
  });
  return walk === "unreadable" ? undefined : code;
}

function occurrences(line: string, token: string): number {
  let count = 0;
  for (let at = line.indexOf(token); at !== -1; at = line.indexOf(token, at + token.length)) {
    count += 1;
  }
  return count;
}

/** `has_unterminated_block FILE`: more `/*` than `*\/` in a regular file. */
export function hasUnterminatedBlock(path: string): boolean {
  if (!isRegularFile(path)) return false;
  let balance = 0;
  eachLine(path, (line) => {
    balance += occurrences(line, "/*") - occurrences(line, "*/");
  });
  return balance > 0;
}
