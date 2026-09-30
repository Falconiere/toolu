/**
 * The line rules (#267): lint suppression (30) and `unsafe` code (40). The
 * unsafe scan drops `//` and `/* *\/` comments first, as the awk did; string
 * literals are not tracked.
 */
import { ere, pathHas, type RsFile } from "./rs-file.ts";

const SUPPRESSION = ere(
  String.raw`^[[:space:]]*#!?\[(allow|expect)\(|^[[:space:]]*#!?\[cfg_attr\([^\]]*\b(allow|expect)\b`,
);
/** In a test file the inner `#![...]` header is accepted. */
const OUTER_SUPPRESSION = ere(
  String.raw`^[[:space:]]*#\[(allow|expect)\(|^[[:space:]]*#\[cfg_attr\([^\]]*\b(allow|expect)\b`,
);

/** 30-suppression. */
export function suppression(f: RsFile): string | undefined {
  const inTests = pathHas(f, "/tests/");
  const pattern = inTests ? OUTER_SUPPRESSION : SUPPRESSION;
  if (!f.lines.some((line) => pattern.test(line))) return undefined;
  const hint = inTests
    ? " In a test file only the file-level #![allow(...)] header is accepted."
    : "";
  return `Forbidden lint suppression (#[allow]/#[expect]/cfg_attr allow) in ${f.file.path} — remove it and fix the underlying warning in code. For unsafe_code, override in Cargo.toml [lints.rust].${hint}`;
}

const UNSAFE = /(^|[^A-Za-z0-9_])unsafe[ \t]*(\{|fn )/s;

/** Whether any line holds `unsafe {` or `unsafe fn` outside a comment. */
function hasUnsafe(lines: readonly string[]): boolean {
  let inBlock = false;
  for (const raw of lines) {
    let line = raw;
    if (inBlock) {
      const close = line.indexOf("*/");
      if (close === -1) continue;
      line = line.slice(close + 2);
      inBlock = false;
    }
    for (let open = line.indexOf("/*"); open !== -1; open = line.indexOf("/*")) {
      const rest = line.slice(open + 2);
      const close = rest.indexOf("*/");
      if (close === -1) {
        line = line.slice(0, open);
        inBlock = true;
        break;
      }
      line = line.slice(0, open) + rest.slice(close + 2);
    }
    const comment = line.indexOf("//");
    if (UNSAFE.test(comment === -1 ? line : line.slice(0, comment))) return true;
  }
  return false;
}

/** 40-unsafe, unless the path names an exempt crate. */
export function unsafeCode(f: RsFile): string | undefined {
  if (f.limits.unsafeExemptions.some((crate) => crate !== "" && pathHas(f, crate)))
    return undefined;
  if (!hasUnsafe(f.lines)) return undefined;
  return `Forbidden unsafe code in ${f.file.path} — refactor to safe alternative. Add crate to settings/rust-unsafe-exemptions.txt if it legitimately needs unsafe (FFI, sandboxing).`;
}
