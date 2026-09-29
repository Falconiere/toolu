/**
 * What a simple command actually runs (#284): wrapper commands are peeled
 * (`sudo -u me timeout 60 git push` runs `git push`), and a shell or `eval`
 * that runs a string names the string to analyze next.
 */
import { basename } from "node:path";
import { hasOption, parseArgs, type OptionSpec } from "./shell-options.ts";

type Words = readonly (string | null)[];

interface Wrapper {
  readonly options: OptionSpec;
  /** Options after which the wrapper runs no command (`command -v`, `sudo -l`). */
  readonly inert?: readonly string[];
  /** Options that hide the command from static reading (`env -S STRING`). */
  readonly opaque?: readonly string[];
  /** `NAME=value` words may sit between the options and the command. */
  readonly assignments?: boolean;
  /** Operands before the command (`timeout DURATION`). */
  readonly operands?: number;
  /** Appends arguments read at run time (`xargs`). */
  readonly appendsDynamic?: boolean;
}

const WRAPPERS: Readonly<Record<string, Wrapper>> = {
  sudo: {
    options: {
      valueShort: "ugCDprtTU",
      valueLong: ["user", "group", "close-from", "chdir", "prompt", "role", "type"],
      stopAtOperand: true,
    },
    inert: [
      "e",
      "l",
      "v",
      "K",
      "V",
      "h",
      "edit",
      "list",
      "validate",
      "remove-timestamp",
      "version",
      "help",
    ],
    assignments: true,
  },
  doas: { options: { valueShort: "uC", stopAtOperand: true }, inert: ["L"] },
  env: {
    options: { valueShort: "uC", valueLong: ["unset", "chdir"], stopAtOperand: true },
    opaque: ["S", "split-string"],
    assignments: true,
  },
  command: { options: { stopAtOperand: true }, inert: ["v", "V"] },
  builtin: { options: { stopAtOperand: true } },
  exec: { options: { valueShort: "a", stopAtOperand: true } },
  nohup: { options: { stopAtOperand: true } },
  time: { options: { valueShort: "fo", valueLong: ["format", "output"], stopAtOperand: true } },
  nice: {
    options: { valueShort: "n", valueLong: ["adjustment"], numeric: true, stopAtOperand: true },
  },
  timeout: {
    options: { valueShort: "sk", valueLong: ["signal", "kill-after"], stopAtOperand: true },
    operands: 1,
  },
  xargs: {
    options: {
      valueShort: "adEILnPs",
      valueLong: [
        "arg-file",
        "delimiter",
        "max-args",
        "max-procs",
        "max-chars",
        "process-slot-var",
      ],
      stopAtOperand: true,
    },
    appendsDynamic: true,
  },
  stdbuf: {
    options: { valueShort: "ioe", valueLong: ["input", "output", "error"], stopAtOperand: true },
  },
};

const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*=/;

export interface Unwrapped {
  readonly argv: readonly (string | null)[];
  /** Aligned with `argv`: the pattern of each word bash globs. */
  readonly patterns: readonly (string | null)[];
  readonly wrappers: readonly string[];
}

/** The wrapper spec for `name`, matched on its basename (`/usr/bin/sudo`). */
function wrapperOf(name: string | null | undefined): Wrapper | undefined {
  if (name === null || name === undefined) return undefined;
  return Object.hasOwn(WRAPPERS, basename(name)) ? WRAPPERS[basename(name)] : undefined;
}

/** Where the wrapped command starts, `"opaque"` when it cannot be read, `null` when none runs. */
function innerStart(words: Words, wrapper: Wrapper): number | "opaque" | null {
  const parsed = parseArgs(words, 1, wrapper.options);
  if (parsed.missingValue || hasOption(parsed, wrapper.inert ?? [])) return null;
  if (hasOption(parsed, wrapper.opaque ?? [])) return "opaque";
  let start = parsed.next;
  while (wrapper.assignments === true && ASSIGNMENT.test(words[start] ?? "")) start += 1;
  start += wrapper.operands ?? 0;
  return start < words.length ? start : null;
}

/** Peel wrapper commands off `words` until the command that actually runs. */
export function unwrap(words: Words, patterns: Words): Unwrapped {
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
    if (inner === "opaque") return { argv: [null], patterns: [null], wrappers };
    appendsDynamic ||= wrapper.appendsDynamic === true;
    start += inner;
  }
  const tail = appendsDynamic ? [null] : [];
  return {
    argv: [...words.slice(start), ...tail],
    patterns: [...patterns.slice(start), ...tail],
    wrappers,
  };
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
