/**
 * What a simple command actually runs (#284): wrapper commands are peeled
 * (`sudo -u me timeout 60 git push` runs `git push`), and a shell or `eval`
 * that runs a string names the string to analyze next.
 */
import { basename } from "node:path";
import { hasOption, parseArgs, type OptionSpec } from "./shell-options.ts";

type Words = readonly (string | null)[];

interface Wrapper extends OptionSpec {
  /** Options after which the wrapper runs no command (`command -v`, `sudo -l`). */
  readonly inert?: string;
  /** Options that hide the command from static reading (`env -S STRING`). */
  readonly opaque?: string;
  /** `NAME=value` words may sit between the options and the command. */
  readonly assignments?: boolean;
  /** Operands before the command (`timeout DURATION`). */
  readonly operands?: number;
  /** Appends arguments read at run time (`xargs`). */
  readonly appendsDynamic?: boolean;
  /** A bare `-` is an option, not the command (`env -` means `env -i`). */
  readonly dashOption?: boolean;
}

/** Option tables; names are space-separated. Parsing stops at the command. */
const WRAPPERS: Readonly<Record<string, Wrapper>> = {
  sudo: {
    valueShort: "ugCDprtTU",
    valueLong: "user group close-from chdir prompt role type",
    inert: "e l v K V h edit list validate remove-timestamp version help",
    assignments: true,
  },
  doas: { valueShort: "uC", inert: "L" },
  env: {
    valueShort: "uCPa",
    valueLong: "unset chdir argv0",
    opaque: "S split-string",
    assignments: true,
    dashOption: true,
  },
  command: { inert: "v V" },
  builtin: {},
  exec: { valueShort: "a" },
  nohup: {},
  time: { valueShort: "fo", valueLong: "format output" },
  nice: { valueShort: "n", valueLong: "adjustment", numeric: true },
  timeout: { valueShort: "sk", valueLong: "signal kill-after", operands: 1 },
  xargs: {
    valueShort: "adEILnPs",
    valueLong: "arg-file delimiter max-args max-procs max-chars process-slot-var",
    appendsDynamic: true,
  },
  stdbuf: { valueShort: "ioe", valueLong: "input output error" },
};

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

export interface Unwrapped {
  readonly wrappers: readonly string[];
  /** Index in the words where the command that runs starts; `null` when it is hidden (`env -S`). */
  readonly start: number | null;
  /** The command gets arguments read at run time (`xargs`). */
  readonly appendsDynamic: boolean;
}

/** The wrapper spec for `name`, matched on its basename (`/usr/bin/sudo`). */
function wrapperOf(name: string | null | undefined): Wrapper | undefined {
  if (name === null || name === undefined) return undefined;
  return Object.hasOwn(WRAPPERS, basename(name)) ? WRAPPERS[basename(name)] : undefined;
}

/** Where the wrapped command starts, `"opaque"` when it cannot be read, `null` when none runs. */
function innerStart(words: Words, wrapper: Wrapper): number | "opaque" | null {
  const parsed = parseArgs(words, 1, { ...wrapper, stopAtOperand: true });
  if (parsed.missingValue || hasOption(parsed, wrapper.inert ?? "")) return null;
  if (hasOption(parsed, wrapper.opaque ?? "")) return "opaque";
  let start = parsed.next;
  for (; start < words.length; start++) {
    const word = words[start] ?? "";
    const dash = wrapper.dashOption === true && word === "-";
    if (!dash && !(wrapper.assignments === true && ASSIGNMENT.test(word))) break;
  }
  start += wrapper.operands ?? 0;
  return start < words.length ? start : null;
}

/** Peel wrapper commands off `words` until the command that actually runs. */
export function unwrap(words: Words): Unwrapped {
  let start = 0;
  let appendsDynamic = false;
  const wrappers: string[] = [];
  for (
    let wrapper = wrapperOf(words[0]);
    wrapper !== undefined;
    wrapper = wrapperOf(words[start])
  ) {
    const inner = innerStart(words.slice(start), wrapper);
    if (inner === null) break;
    wrappers.push(basename(words[start] ?? ""));
    if (inner === "opaque") return { wrappers, start: null, appendsDynamic };
    appendsDynamic ||= wrapper.appendsDynamic === true;
    start += inner;
  }
  return { wrappers, start, appendsDynamic };
}

/** `list` (aligned with the words) cut to the command that runs; `fill` stands for unknown words. */
export function alignUnwrapped<T>(list: readonly T[], unwrapped: Unwrapped, fill: T): T[] {
  if (unwrapped.start === null) return [fill];
  return [...list.slice(unwrapped.start), ...(unwrapped.appendsDynamic ? [fill] : [])];
}

const SHELLS = new Set(["bash", "sh", "zsh", "dash", "ksh"]);

/** A string that a command runs as shell code. */
export interface RunTarget {
  readonly origin: "shell" | "eval";
  /** The code, or `null` when it is dynamic. */
  readonly script: string | null;
  /** The shell reads its script from stdin (no `-c`, no script operand). */
  readonly stdin: boolean;
}

function shellTarget(argv: Words): RunTarget | null {
  let command = false;
  let stdin = false;
  let i = 1;
  for (; i < argv.length; i++) {
    const word = argv[i] ?? null;
    if (word === null) return { origin: "shell", script: null, stdin: false };
    if (word === "--" || word === "-") {
      i += 1;
      break;
    }
    if (word.startsWith("--")) {
      if (word === "--rcfile" || word === "--init-file") i += 1;
      continue;
    }
    if (!/^[-+]./.test(word)) break;
    const letters = word.slice(1);
    command ||= word.startsWith("-") && letters.includes("c");
    stdin ||= word.startsWith("-") && letters.includes("s");
    if (/[oO]/.test(letters)) i += 1;
  }
  if (command)
    return i < argv.length ? { origin: "shell", script: argv[i] ?? null, stdin: false } : null;
  if (stdin || i >= argv.length) return { origin: "shell", script: null, stdin: true };
  return null;
}

/** The shell code `argv` runs as a string (`bash -c`, a shell on stdin, `eval`), if any. */
export function runTarget(argv: Words): RunTarget | null {
  const name = argv[0];
  if (name === null || name === undefined) return null;
  if (SHELLS.has(basename(name))) return shellTarget(argv);
  if (name !== "eval") return null;
  const args = argv[1] === "--" ? argv.slice(2) : argv.slice(1);
  if (args.length === 0) return null;
  const script = args.some((arg) => arg === null) ? null : args.join(" ");
  return { origin: "eval", script, stdin: false };
}
