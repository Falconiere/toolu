import { mkdir, readFile, realpath, rename, writeFile } from "node:fs/promises";
import { dirname, join, basename } from "node:path";
import {
  applyEdits,
  createScanner,
  findNodeAtLocation,
  modify,
  parse,
  parseTree,
  printParseErrorCode,
  type JSONPath,
  type Node,
  type ParseError,
} from "jsonc-parser/lib/esm/main.js"; // the UMD entry's dynamic requires break the Node bundle
import { CliError, EXIT } from "../exit";

/** One OpenCode config file: absent files have neither text nor data. */
export interface ConfigFile {
  readonly path: string;
  readonly text: string | undefined;
  readonly data: Readonly<Record<string, unknown>> | undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isMissing(error: unknown): boolean {
  return error instanceof Error && "code" in error && error.code === "ENOENT";
}

/** Parses JSONC text, failing closed with the file name on any syntax error. */
export function parseConfig(path: string, text: string): Readonly<Record<string, unknown>> {
  const errors: ParseError[] = [];
  const data: unknown = parse(text, errors, { allowTrailingComma: true });
  const first = errors[0];
  if (first !== undefined) {
    throw new CliError(
      EXIT.failed,
      `${path} is not valid JSONC (${printParseErrorCode(first.error)} at offset ${first.offset}); nothing was written`,
    );
  }
  if (!isRecord(data)) {
    throw new CliError(EXIT.failed, `${path} must hold a JSON object; nothing was written`);
  }
  return data;
}

/** Reads a config file; a path that does not exist is absent, anything unreadable fails. */
export async function readConfigFile(path: string): Promise<ConfigFile> {
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    if (isMissing(error)) return { path, text: undefined, data: undefined };
    throw new CliError(EXIT.failed, `cannot read ${path}: ${String(error)}`);
  }
  return { path, text, data: parseConfig(path, text) };
}

/** The file's own indentation and line ending, so an edit reads like the rest of it. */
function formattingOf(text: string): { insertSpaces: boolean; tabSize: number; eol: string } {
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const indent = /^([ \t]+)\S/m.exec(text)?.[1] ?? "  ";
  if (indent.startsWith("\t")) return { insertSpaces: false, tabSize: 1, eol };
  return { insertSpaces: true, tabSize: indent.length, eol };
}

/**
 * Sets (or, with `undefined`, removes) the value at `path`, keeping comments and
 * formatting elsewhere. `insert` adds a new array element at the given index.
 */
export function editJsonc(text: string, path: JSONPath, value: unknown, insert = false): string {
  const edits = modify(text, path, value, {
    formattingOptions: formattingOf(text),
    isArrayInsertion: insert,
  });
  return applyEdits(text, edits);
}

function arrayAt(text: string, key: string): Node | undefined {
  const root = parseTree(text, [], { allowTrailingComma: true });
  const node = root === undefined ? undefined : findNodeAtLocation(root, [key]);
  return node?.type === "array" ? node : undefined;
}

function lineStart(text: string, offset: number): number {
  return text.lastIndexOf("\n", offset - 1) + 1;
}

/** Whether only whitespace precedes `offset` on its line. */
function ownLine(text: string, offset: number): boolean {
  return text.slice(lineStart(text, offset), offset).trim() === "";
}

/** Offset of the first comma token in `text[from, to)`, skipping comments. */
function commaIn(text: string, from: number, to: number): number | undefined {
  const slice = text.slice(from, to);
  const scanner = createScanner(slice, false);
  while (scanner.getPosition() < slice.length) {
    scanner.scan();
    const offset = scanner.getTokenOffset();
    if (slice.slice(offset, offset + scanner.getTokenLength()) === ",") return from + offset;
  }
  return undefined;
}

/**
 * Appends `value` to the array at `key` on a line of its own, after any comment
 * trailing the last element, so the user's comments stay on their own entries.
 */
export function appendJsonc(text: string, key: string, value: unknown): string {
  const array = arrayAt(text, key);
  const last = array?.children?.at(-1);
  if (array === undefined || last === undefined || !ownLine(text, last.offset)) {
    return editJsonc(text, [key, array?.children?.length ?? 0], value, true);
  }
  const close = array.offset + array.length - 1;
  const lastEnd = last.offset + last.length;
  // Before the line break ending the last element's line, or before `]` sharing that line.
  let at = close;
  if (lineStart(text, close) > lastEnd) {
    at = lineStart(text, close) - 1;
    if (text[at - 1] === "\r") at -= 1;
  }
  const trailing = commaIn(text, lastEnd, close) !== undefined;
  const indent = text.slice(lineStart(text, last.offset), last.offset);
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const entry = `${eol}${indent}${JSON.stringify(value)}${trailing ? "," : ""}`;
  const head = trailing
    ? text.slice(0, at)
    : `${text.slice(0, lastEnd)},${text.slice(lastEnd, at)}`;
  return head + entry + text.slice(at);
}

/**
 * Removes element `index` of the array at `key`. An element on its own line
 * goes with its line (and its trailing comment); neighbours keep theirs.
 */
export function removeJsonc(text: string, key: string, index: number): string {
  const array = arrayAt(text, key);
  const element = array?.children?.[index];
  if (array === undefined || element === undefined || !ownLine(text, element.offset)) {
    return editJsonc(text, [key, index], undefined);
  }
  const elementEnd = element.offset + element.length;
  const next = array.children?.[index + 1];
  const limit = next?.offset ?? array.offset + array.length - 1;
  const comma = commaIn(text, elementEnd, limit);
  const newline = text.indexOf("\n", comma ?? elementEnd);
  const sharesLine = newline === -1 || newline >= limit;
  if (sharesLine && next !== undefined) return editJsonc(text, [key, index], undefined);
  const body =
    text.slice(0, lineStart(text, element.offset)) + text.slice(sharesLine ? limit : newline + 1);
  // The new last element must not keep a comma the removed one followed.
  const previous = array.children?.[index - 1];
  if (comma !== undefined || next !== undefined || previous === undefined) return body;
  const before = commaIn(text, previous.offset + previous.length, element.offset);
  return before === undefined ? body : body.slice(0, before) + body.slice(before + 1);
}

/** A new config holding only the schema and the given plugin entry. */
export function newConfigText(spec: string): string {
  return `${JSON.stringify({ $schema: "https://opencode.ai/config.json", plugin: [spec] }, null, 2)}\n`;
}

/** The path a write lands on: a symlink's target, so the link itself survives. */
async function writeTarget(path: string): Promise<string> {
  try {
    return await realpath(path);
  } catch (error) {
    if (isMissing(error)) return path;
    throw error;
  }
}

/** Writes through a temp file and rename in the target's directory. */
export async function writeAtomic(path: string, text: string): Promise<void> {
  const target = await writeTarget(path);
  await mkdir(dirname(target), { recursive: true });
  const temp = join(dirname(target), `.${basename(target)}.${process.pid}.tmp`);
  await writeFile(temp, text, "utf8");
  await rename(temp, target);
}
