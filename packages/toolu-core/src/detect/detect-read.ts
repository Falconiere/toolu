/**
 * Bounded line reading for the detect layer (#254). A file is read in fixed
 * chunks, so memory stays flat whatever its size, and decoded as latin1 (one
 * char per byte), so the awk and grep byte semantics the bash functions had
 * hold for any encoding.
 */
import { closeSync, openSync, readSync, statSync } from "node:fs";

const CHUNK = 64 * 1024;
const NEWLINE = 0x0a;

/** A regular file (symlinks followed), bash `[ -f ]`. */
export function isRegularFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** How a walk ended: every line seen, stopped by the visitor, or the file could not be read. */
export type LineWalk = "done" | "stopped" | "unreadable";

/**
 * Call `visit` with each line of `path`, as awk splits records: on `\n`, with a
 * final unterminated line counted and no empty record after a trailing
 * newline. `visit` returns `true` to stop early. A file that cannot be opened
 * or read is `unreadable` (awk prints nothing then).
 */
export function eachLine(path: string, visit: (line: string) => boolean | void): LineWalk {
  let fd: number;
  try {
    fd = openSync(path, "r");
  } catch {
    return "unreadable";
  }
  try {
    return walkLines(fd, visit);
  } catch (error) {
    // awk reads a directory as an empty file.
    return isErrno(error, "EISDIR") ? "done" : "unreadable";
  } finally {
    closeSync(fd);
  }
}

function isErrno(error: unknown, code: string): boolean {
  return error instanceof Error && "code" in error && error.code === code;
}

/** Lines are sliced straight from the chunk; only a line spanning chunks is carried. */
function walkLines(fd: number, visit: (line: string) => boolean | void): LineWalk {
  const buf = Buffer.allocUnsafe(CHUNK);
  let carry = "";
  for (let n = readSync(fd, buf, 0, CHUNK, null); n > 0; n = readSync(fd, buf, 0, CHUNK, null)) {
    let start = 0;
    for (let nl = buf.indexOf(NEWLINE, 0); nl !== -1 && nl < n; nl = buf.indexOf(NEWLINE, start)) {
      const line = carry + buf.toString("latin1", start, nl);
      carry = "";
      start = nl + 1;
      if (visit(line) === true) return "stopped";
    }
    carry += buf.toString("latin1", start, n);
  }
  if (carry !== "" && visit(carry) === true) return "stopped";
  return "done";
}
