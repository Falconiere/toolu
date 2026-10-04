import { mkdir, readFile, realpath, rename, writeFile } from "node:fs/promises";
import { dirname, join, basename } from "node:path";
import {
  applyEdits,
  modify,
  parse,
  printParseErrorCode,
  type JSONPath,
  type ParseError,
} from "jsonc-parser";
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
