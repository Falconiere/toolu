/**
 * Does a source file silence dead-code enforcement? Lines are sanitized by the
 * lexers first, so only real comments (script) or real attributes (Rust) can
 * match. A prose comment stays legal: the directive must be the first token
 * after the comment delimiter.
 */
import { newRustState, rustSyntaxLine } from "./rust.ts";
import { scriptSyntaxLine } from "./script.ts";
import { newScriptState } from "./script-state.ts";

const S = "[ \\t\\n\\v\\f\\r]";
const DIRECTIVE = "(oxlint|eslint)-disable(-next-line|-line)?";
const UNUSED = "((eslint|typescript|@typescript-eslint)/)?no-unused-vars";

const LINE_BLANKET = new RegExp(`//${S}*${DIRECTIVE}${S}*(--[^\\x00-\\x1f\\x7f]*)?$`);
const LINE_UNUSED = new RegExp(`//${S}*${DIRECTIVE}${S}+([^,${S.slice(1, -1)}]+${S}*,${S}*)*${UNUSED}([${S.slice(1, -1)},]|$)`);
const BLOCK_START = new RegExp(`/\\*${S}*(oxlint|eslint)-disable`);
const BLOCK_BLANKET = new RegExp(`/\\*${S}*${DIRECTIVE}${S}*(--[^*]*)?\\*/`);
const BLOCK_UNUSED = new RegExp(`/\\*${S}*${DIRECTIVE}${S}+([^,${S.slice(1, -1)}*]+${S}*,${S}*)*${UNUSED}([${S.slice(1, -1)},*]|$)`);
const RUST_DIRECT = /^#!?\[(r#)?(allow|warn|expect)\(([^,)]*,)*(r#)?(dead_code|unused)(,[^)]*)?\)\]/;
const RUST_CFG_ATTR = /^#!?\[(r#)?cfg_attr\(.*,(r#)?(allow|warn|expect)\(([^,)]*,)*(r#)?(dead_code|unused)(,[^)]*)?\).*\)\]/;

/** Cheap whole-file prefilters: no candidate text, no lexing. */
export const SCRIPT_CANDIDATE = /(oxlint|eslint)-disable/;
export const RUST_CANDIDATE = /(^|[^\p{L}\p{Nd}_])(allow|warn|expect)([^\p{L}\p{Nd}_]|$)/u;

/** Physical lines as `read -r` sees them: a trailing newline adds no empty line. */
export function physicalLines(text: string): string[] {
  if (text === "") return [];
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return lines;
}

function blockForbidden(block: string): boolean {
  return BLOCK_BLANKET.test(block) || BLOCK_UNUSED.test(block);
}

export function scriptForbidden(path: string, text: string): boolean {
  const st = newScriptState(path);
  st.source = text.replace(/\n+$/, "");
  let offset = 0;
  let block = "";
  let inBlock = false;
  for (const raw of physicalLines(text)) {
    st.sourceOffset = offset;
    const line = scriptSyntaxLine(st, raw);
    offset += raw.length + 1;
    if (inBlock) {
      block = `${block} ${line}`;
      if (line.includes("*/")) {
        if (blockForbidden(block)) return true;
        block = "";
        inBlock = false;
      }
      continue;
    }
    if (LINE_BLANKET.test(line) || LINE_UNUSED.test(line)) return true;
    if (BLOCK_START.test(line)) {
      block = line;
      const after = line.slice(line.indexOf("/*") + 2);
      if (!after.includes("*/")) inBlock = true;
      else if (blockForbidden(block)) return true;
      else block = "";
    }
  }
  return false;
}

function bracketDepth(line: string, depth: number): number {
  let next = depth;
  for (const char of line) {
    if (char === "[") next += 1;
    else if (char === "]") next -= 1;
  }
  return next;
}

function rustAttrForbidden(attr: string): boolean {
  const compact = attr.replace(/[ \t\n\v\f\r]/g, "");
  return RUST_DIRECT.test(compact) || RUST_CFG_ATTR.test(compact);
}

/** Collect an attribute through its closing bracket before matching it. */
export function rustForbidden(text: string): boolean {
  const st = newRustState();
  let attr = "";
  let depth = 0;
  let inAttr = false;
  for (const raw of physicalLines(text)) {
    const line = rustSyntaxLine(st, raw);
    if (inAttr) {
      attr = `${attr} ${line}`;
      depth = bracketDepth(line, depth);
      if (depth <= 0) {
        if (rustAttrForbidden(attr)) return true;
        attr = "";
        inAttr = false;
        depth = 0;
      }
      continue;
    }
    const trimmed = line.replace(/^[ \t\n\v\f\r]+/, "");
    if (!trimmed.startsWith("#[") && !trimmed.startsWith("#![")) continue;
    attr = trimmed;
    depth = bracketDepth(trimmed, 0);
    if (depth > 0) {
      inAttr = true;
      continue;
    }
    if (rustAttrForbidden(attr)) return true;
    attr = "";
    depth = 0;
  }
  return false;
}
