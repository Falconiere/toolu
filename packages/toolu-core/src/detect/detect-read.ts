/**
 * Bounded line reading for the detect layer (#254). A file is read in fixed
 * chunks, so memory stays flat whatever its size, and decoded as latin1 (one
 * char per byte), so the awk and grep byte semantics the bash functions had
 * hold for any encoding.
 */
import { closeSync, openSync, readSync, statSync } from "node:fs";

const CHUNK = 64 * 1024;

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
    const buf = Buffer.allocUnsafe(CHUNK);
    let carry = "";
    for (;;) {
      const n = readSync(fd, buf, 0, CHUNK, null);
      if (n === 0) break;
      const lines = (carry + buf.toString("latin1", 0, n)).split("\n");
      carry = lines.pop() ?? "";
      for (const line of lines) {
        if (visit(line) === true) return "stopped";
      }
    }
    if (carry !== "" && visit(carry) === true) return "stopped";
    return "done";
  } catch {
    return "unreadable";
  } finally {
    closeSync(fd);
  }
}
