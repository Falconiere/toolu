/**
 * Files a command line writes (#284): output redirections on any command or
 * compound command, and the file operands of commands that write in place or
 * copy. A dynamic target is reported with `path: null` rather than dropped, and
 * an unquoted pathname pattern (`> .en[v]`, `cp x .e?v`) with its `pattern`:
 * bash writes whichever existing file the pattern matches.
 */
import { basename } from "node:path";
import { hasOption, named, optionValues, parseArgs, type OptionSpec } from "./shell-options.ts";
import type { ShellAnalysis, ShellCommand, ShellRedirect } from "./shell-types.ts";

type Words = readonly (string | null)[];

export type WriteVia =
  | "redirect"
  | "tee"
  | "sed"
  | "perl"
  | "cp"
  | "mv"
  | "install"
  | "dd"
  | "python";

export interface WriteTarget {
  /** The written path's static value, or `null` when it is dynamic or a pathname pattern. */
  readonly path: string | null;
  /**
   * An unquoted pathname pattern: bash writes the one existing file it matches,
   * or the literal name when none does. A guardrail must treat it as every path
   * it matches.
   */
  readonly pattern: string | null;
  /** The target with quotes removed and expansions as written (`$HOME/.env`), for display and name matching. */
  readonly text: string;
  readonly via: WriteVia;
  /** The command that writes; `null` for a redirect on a compound command. */
  readonly command: ShellCommand | null;
}

/** A written file: its static path, or the pathname pattern bash expands for it. */
interface Target {
  readonly path: string | null;
  readonly pattern: string | null;
  /** Quotes removed, expansions as written. */
  readonly text: string;
}

const WRITE_OPERATORS: ReadonlySet<string> = new Set([">", ">>", ">|", "&>", "&>>", "<>"]);

/** `>`, `>>`, `>|`, `&>`, `&>>`, `<>`, and `>&` onto a file rather than a descriptor (`2>&1`, `2>&-`). */
function writesFile(redirect: ShellRedirect): boolean {
  if (WRITE_OPERATORS.has(redirect.operator)) return true;
  if (redirect.operator !== ">&") return false;
  return redirect.target === null || !/^(\d+|-)$/.test(redirect.target);
}

function redirectTargets(
  redirects: readonly ShellRedirect[],
  command: ShellCommand | null,
): WriteTarget[] {
  return redirects.filter(writesFile).map((redirect) => ({
    path: redirect.target,
    pattern: redirect.pattern,
    text: redirect.text,
    via: "redirect",
    command,
  }));
}

function argAt(command: ShellCommand, index: number): Target {
  return {
    path: command.argv[index] ?? null,
    pattern: command.patterns[index] ?? null,
    text: command.texts[index] ?? "",
  };
}

function join(dir: string, source: string): string {
  return `${dir.replace(/\/+$/, "")}/${basename(source)}`;
}

function inDir(dir: Target, source: Target): Target {
  const text = join(dir.text, source.text);
  if (dir.path !== null && source.path !== null)
    return { path: join(dir.path, source.path), pattern: null, text };
  const d = dir.path ?? dir.pattern;
  const s = source.path ?? source.pattern;
  return { path: null, pattern: d === null || s === null ? null : join(d, s), text };
}

const MOVE: OptionSpec = { valueShort: "tS", valueLong: "target-directory suffix" };
const COPY_OPTIONS: Readonly<Record<string, OptionSpec>> = {
  cp: MOVE,
  mv: MOVE,
  install: {
    valueShort: "tSmog",
    valueLong: "target-directory suffix mode owner group strip-program",
  },
};

/** cp/mv/install: the destination and each source inside it, or each source inside `-t DIR`. */
function copyTargets(name: string, command: ShellCommand): Target[] {
  const parsed = parseArgs(command.argv, 1, COPY_OPTIONS[name] ?? {});
  const operands = parsed.operandAt.map((index) => argAt(command, index));
  if (name === "install" && hasOption(parsed, "d directory")) return operands;
  const dirOption = parsed.options.find((o) => named("t target-directory", o.name));
  if (dirOption !== undefined) {
    const value = dirOption.value ?? null;
    const dir: Target =
      dirOption.at === null
        ? { path: value, pattern: null, text: value ?? "" }
        : argAt(command, dirOption.at);
    return operands.map((source) => inDir(dir, source));
  }
  const sources = operands.slice(0, -1);
  const dest = operands.at(-1);
  if (dest === undefined || sources.length === 0) return [];
  if (dest.path === null && dest.pattern === null) return [dest];
  // DEST may be an existing directory (only known at run time), so each
  // DEST/basename(SRC) is a candidate too: `cp .env.example apps/web` writes
  // apps/web/.env.example.
  return [dest, ...sources.map((source) => inDir(dest, source))];
}

interface InPlaceTool {
  readonly options: OptionSpec;
  /** Options that supply the script, so the first operand is already a file. */
  readonly script: string;
}

const IN_PLACE: Readonly<Record<"sed" | "perl", InPlaceTool>> = {
  sed: {
    options: { valueShort: "efl", restShort: "i", valueLong: "expression file line-length" },
    script: "e f expression file",
  },
  perl: { options: { valueShort: "eE", restShort: "iIMmlx0dDC" }, script: "e E" },
};

/** sed/perl edit files only in place; their first operand is the script unless an option gave one. */
function inPlaceTargets(name: "sed" | "perl", command: ShellCommand): Target[] {
  const tool = IN_PLACE[name];
  const parsed = parseArgs(command.argv, 1, tool.options);
  if (!hasOption(parsed, "i in-place")) return [];
  // BSD `sed -i '' …`: the empty word is the backup suffix, not the script.
  const argv = command.argv;
  const bsd = argv.findIndex((word, i) => word === "-i" && argv[i + 1] === "");
  const files = parsed.operandAt.filter((index) => bsd === -1 || index !== bsd + 1);
  return files.slice(hasOption(parsed, tool.script) ? 0 : 1).map((index) => argAt(command, index));
}

/** `open('p', 'w')` and every mode that writes: w/a/x, or anything with `+`. */
const PYTHON_OPEN = /open\(\s*['"]([^'"]+)['"]\s*,\s*['"]([wax][^'"]*|r[^'"]*\+[^'"]*)['"]/g;

function pythonTargets(argv: Words): Target[] {
  const parsed = parseArgs(argv, 1, { valueShort: "cmWX", stopAtOperand: true });
  const [script] = optionValues(parsed, "c");
  if (script === null || script === undefined) return [];
  return [...script.matchAll(PYTHON_OPEN)].flatMap((match) =>
    match[1] === undefined ? [] : [{ path: match[1], pattern: null, text: match[1] }],
  );
}

/** `dd of=FILE`; the whole `of=…` word never names an existing file, so it is not globbed. */
function ddTargets(command: ShellCommand): Target[] {
  return command.texts.flatMap((text, index) => {
    if (!text.startsWith("of=")) return [];
    const word = command.argv[index] ?? command.patterns[index] ?? null;
    return [
      {
        path: word?.startsWith("of=") === true ? word.slice(3) : null,
        pattern: null,
        text: text.slice(3),
      },
    ];
  });
}

function argumentTargets(command: ShellCommand): { via: WriteVia; targets: Target[] } | null {
  const first = command.argv[0];
  const name = first === null || first === undefined ? "" : basename(first);
  if (name === "tee") {
    const parsed = parseArgs(command.argv, 1, {});
    return { via: "tee", targets: parsed.operandAt.map((index) => argAt(command, index)) };
  }
  if (name === "sed" || name === "perl")
    return { via: name, targets: inPlaceTargets(name, command) };
  if (name === "cp" || name === "mv" || name === "install")
    return { via: name, targets: copyTargets(name, command) };
  if (name === "dd") return { via: "dd", targets: ddTargets(command) };
  if (/^python[0-9.]*$/.test(name)) return { via: "python", targets: pythonTargets(command.argv) };
  return null;
}

/** Every write target in the line: redirects first, then argument-derived paths, per command. */
export function writeTargets(analysis: ShellAnalysis): WriteTarget[] {
  const targets = analysis.commands.flatMap((command) => {
    const found = argumentTargets(command);
    const fromArgs =
      found === null
        ? []
        : found.targets.map(({ path, pattern, text }) => ({
            path,
            pattern,
            text,
            via: found.via,
            command,
          }));
    return [...redirectTargets(command.redirects, command), ...fromArgs];
  });
  return [...targets, ...redirectTargets(analysis.compoundRedirects, null)];
}
