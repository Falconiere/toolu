/**
 * file-size — the per-file code-line ceiling. Counts CODE lines only (blank
 * and comment-only lines excluded, like oxlint's max-lines with skipBlankLines
 * + skipComments). fileSize.skipExtensions lists what a linter already owns,
 * so on the TypeScript stacks this is a deliberate no-op for .ts/.tsx.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext, Mode } from "../context.ts";
import { matchGlob } from "../glob.ts";
import { findFiles, isDir, isFile } from "../walk.ts";

/**
 * Code lines in `text`, or null for an empty file (awk sees no record and
 * prints nothing, so an empty file is never judged).
 */
export function codeLines(text: string): number | null {
  if (text === "") return null;
  const records = text.split("\n");
  if (text.endsWith("\n")) records.pop();
  let count = 0;
  let inBlock = false;
  for (const record of records) {
    const line = record.replace(/^[ \t]+/, "");
    if (line === "" || line.startsWith("//")) continue;
    // "# …" is a comment in shell/yaml/toml, but "#[derive(…)]" and "#![…]" are
    // Rust ATTRIBUTES — code, and dense in exactly the stack where this counts.
    if (line.startsWith("#") && !/^#!?\[/.test(line)) continue;
    if (line.startsWith("/*")) inBlock = true;
    if (inBlock) {
      if (line.includes("*/")) inBlock = false;
      continue;
    }
    if (line.startsWith("*") || line === "---") continue;
    count += 1;
  }
  return count;
}

function ceilingFor(config: GuardrailsConfig, path: string): number {
  for (const [glob, max] of config.sizeOverrides) {
    if (matchGlob(glob, path)) return max;
  }
  return config.fileMax;
}

/** Ours to measure: not a linter-owned extension, not a test file. */
function eligible(config: GuardrailsConfig, path: string): boolean {
  if (config.skipExtensions.some((ext) => path.endsWith(ext))) return false;
  // Both test shapes: nested (src/parser/tests/lexer.rs) and crate-root (tests/cli.rs).
  return !path.includes(`/${config.testDir}/`) && !path.startsWith(`${config.testDir}/`);
}

/** File text, or null when unreadable: awk reported it on stderr and counted nothing. */
function readable(root: string, path: string): string | null {
  try {
    return readFileSync(resolve(root, path), "latin1");
  } catch {
    return null;
  }
}

function judge(ctx: CheckContext<GuardrailsConfig>, path: string): void {
  const text = readable(ctx.root, path);
  if (text === null) return;
  const lines = codeLines(text);
  if (lines === null) return;
  const ceiling = ceilingFor(ctx.config, path);
  if (lines > ceiling) {
    ctx.report.violation(
      "file-size",
      path,
      `${String(lines)} code lines exceeds the ceiling of ${String(ceiling)}`,
      "split it — promote the file to a folder and move the sub-parts into their own files",
    );
  }
}

export function fileSize(ctx: CheckContext<GuardrailsConfig>, mode: Mode, path: string): void {
  if (mode === "file") {
    if (isFile(ctx.root, path) && eligible(ctx.config, path)) judge(ctx, path);
    return;
  }
  if (!isDir(ctx.root, ctx.config.srcRoot)) return;
  for (const file of findFiles(ctx.root, ctx.config.srcRoot)) {
    if (eligible(ctx.config, file)) judge(ctx, file);
  }
}
