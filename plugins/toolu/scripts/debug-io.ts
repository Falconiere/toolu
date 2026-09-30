/** Shared CLI plumbing for the debug skill's Observe helpers (debug-log,
 * debug-stack, debug-testfail): `[--file <path>] [--json]` parsing, byte input
 * and output, and awk-style numeric caps.
 *
 * Input is handled as a latin1 "binary" string — one char per byte — and
 * written back the same way, so the helpers work on bytes as awk does:
 * invalid UTF-8 passes through untouched and `length` counts bytes. */

import { readFileSync } from "node:fs";

type Summarizer = (lines: string[], json: boolean) => string;

type HelperSpec = {
  /** Script name used in messages, e.g. `debug-log.ts`. */
  name: string;
  usage: string;
  /** What to pipe in, for the no-input message: `a log`, `a trace`, ... */
  inputNoun: string;
  summarize: Summarizer;
};

/** Numeric value of an env cap as awk's `-v var=...` reads it: `${VAR:-default}`,
 * then the leading decimal number, else 0. */
export function envCap(name: string, fallback: number): number {
  const raw = process.env[name] || String(fallback);
  const m = /^[ \t\n]*[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?/.exec(raw);
  return m === null ? 0 : Number(m[0]);
}

/** awk records: split on "\n"; a trailing newline does not open an empty record. */
function records(text: string): string[] {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  return lines;
}

/** A JSON array of strings; `clean` applies the per-helper escaping transform first. */
export function jsonList(items: string[], clean: (s: string) => string): string {
  return `[${items.map((item) => JSON.stringify(clean(item))).join(",")}]`;
}

/** The byte sequence of a UTF-8 literal as it appears in a latin1 input string. */
export function utf8Bytes(literal: string): string {
  return Buffer.from(literal, "utf8").toString("latin1");
}

type Parsed = { file: string; json: boolean } | { exit: number };

function parseArgs(spec: HelperSpec, argv: string[]): Parsed {
  let file = "";
  let json = false;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--file") {
      const value = argv[i + 1];
      if (value === undefined) {
        process.stderr.write(`${spec.name}: --file needs a path\n`);
        return { exit: 2 };
      }
      file = value;
      i++;
    } else if (arg === "--json") {
      json = true;
    } else if (arg === "-h" || arg === "--help") {
      process.stdout.write(spec.usage);
      return { exit: 0 };
    } else {
      process.stderr.write(`${spec.name}: unknown arg: ${arg}\n${spec.usage}`);
      return { exit: 2 };
    }
  }
  return { file, json };
}

function readInput(spec: HelperSpec, file: string): Buffer | { exit: number } {
  if (file !== "") {
    try {
      return readFileSync(file);
    } catch {
      process.stderr.write(`${spec.name}: cannot read file: ${file}\n`);
      return { exit: 2 };
    }
  }
  if (process.stdin.isTTY) {
    process.stderr.write(`${spec.name}: no input (pipe ${spec.inputNoun} or pass --file)\n`);
    return { exit: 2 };
  }
  return readFileSync(0);
}

/** Parse argv, read the input, print the summary; returns the exit code. */
export function runHelper(spec: HelperSpec, argv: string[]): number {
  const parsed = parseArgs(spec, argv);
  if ("exit" in parsed) return parsed.exit;
  const input = readInput(spec, parsed.file);
  if ("exit" in input) return input.exit;
  const out = spec.summarize(records(input.toString("latin1")), parsed.json);
  process.stdout.write(Buffer.from(out, "latin1"));
  return 0;
}
