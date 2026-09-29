/**
 * Files a command line writes (#284): output redirections on any command or
 * compound command, and the file operands of commands that write in place or
 * copy. A dynamic target is reported with `path: null` rather than dropped.
 */
import { basename } from "node:path";
import { hasOption, optionValues, parseArgs, type OptionSpec } from "./shell-options.ts";
import type { ShellAnalysis, ShellCommand, ShellRedirect } from "./shell-types.ts";

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
  /** The written path's static value, or `null` when it is dynamic. */
  readonly path: string | null;
  /** A redirect target as written; for an argument, its static value (empty when dynamic). */
  readonly text: string;
  readonly via: WriteVia;
  /** The command that writes; `null` for a redirect on a compound command. */
  readonly command: ShellCommand | null;
}

type Words = readonly (string | null)[];

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
  return redirects
    .filter(writesFile)
    .map((redirect) => ({ path: redirect.target, text: redirect.text, via: "redirect", command }));
}

function inDir(dir: string, source: string | null): string | null {
  return source === null ? null : `${dir.replace(/\/+$/, "")}/${basename(source)}`;
}

const COPY_OPTIONS: Readonly<Record<string, OptionSpec>> = {
  cp: { valueShort: "tS", valueLong: ["target-directory", "suffix"] },
  mv: { valueShort: "tS", valueLong: ["target-directory", "suffix"] },
  install: {
    valueShort: "tSmog",
    valueLong: ["target-directory", "suffix", "mode", "owner", "group", "strip-program"],
  },
};

/** cp/mv/install: the destination, or each source inside a target directory. */
function copyTargets(name: string, argv: Words): (string | null)[] {
  const parsed = parseArgs(argv, 1, COPY_OPTIONS[name] ?? {});
  if (name === "install" && hasOption(parsed, ["d", "directory"])) return [...parsed.operands];
  const [dir] = optionValues(parsed, ["t", "target-directory"]);
  if (dir !== undefined)
    return dir === null ? [null] : parsed.operands.map((src) => inDir(dir, src));
  const sources = parsed.operands.slice(0, -1);
  const dest = parsed.operands.at(-1);
  if (dest === undefined || sources.length === 0) return [];
  if (dest === null) return [null];
  const intoDir = sources.length > 1 || dest.endsWith("/");
  return [dest, ...(intoDir ? sources.map((src) => inDir(dest, src)) : [])];
}

interface InPlaceTool {
  readonly options: OptionSpec;
  /** Options that supply the script, so the first operand is already a file. */
  readonly script: readonly string[];
}

const IN_PLACE: Readonly<Record<"sed" | "perl", InPlaceTool>> = {
  sed: {
    options: {
      valueShort: "efl",
      restShort: "i",
      valueLong: ["expression", "file", "line-length"],
    },
    script: ["e", "f", "expression", "file"],
  },
  perl: { options: { valueShort: "eE", restShort: "iIMmlx0dDC" }, script: ["e", "E"] },
};

/** sed/perl edit files only in place; their first operand is the script unless an option gave one. */
function inPlaceTargets(name: "sed" | "perl", argv: Words): (string | null)[] {
  // BSD `sed -i '' …`: the empty word is the backup suffix, not the script.
  const bsd = argv.findIndex((word, i) => word === "-i" && argv[i + 1] === "");
  const words = bsd === -1 ? argv : [...argv.slice(0, bsd + 1), ...argv.slice(bsd + 2)];
  const tool = IN_PLACE[name];
  const parsed = parseArgs(words, 1, tool.options);
  if (!hasOption(parsed, ["i", "in-place"])) return [];
  return parsed.operands.slice(hasOption(parsed, tool.script) ? 0 : 1);
}

/** `open('p', 'w')` and every mode that writes: w/a/x, or anything with `+`. */
const PYTHON_OPEN = /open\(\s*['"]([^'"]+)['"]\s*,\s*['"]([wax][^'"]*|r[^'"]*\+[^'"]*)['"]/g;

function pythonTargets(argv: Words): string[] {
  const [script] = optionValues(parseArgs(argv, 1, { valueShort: "cmWX", stopAtOperand: true }), [
    "c",
  ]);
  if (script === null || script === undefined) return [];
  return [...script.matchAll(PYTHON_OPEN)].flatMap((match) =>
    match[1] === undefined ? [] : [match[1]],
  );
}

function argumentTargets(argv: Words): { via: WriteVia; paths: (string | null)[] } | null {
  const name = argv[0] === null || argv[0] === undefined ? "" : basename(argv[0]);
  if (name === "tee") return { via: "tee", paths: [...parseArgs(argv, 1, {}).operands] };
  if (name === "sed" || name === "perl") return { via: name, paths: inPlaceTargets(name, argv) };
  if (name === "cp" || name === "mv" || name === "install")
    return { via: name, paths: copyTargets(name, argv) };
  if (name === "dd") {
    const paths = argv.flatMap((word) => (word?.startsWith("of=") === true ? [word.slice(3)] : []));
    return { via: "dd", paths };
  }
  if (/^python[0-9.]*$/.test(name)) return { via: "python", paths: pythonTargets(argv) };
  return null;
}

/** Every write target in the line: redirects first, then argument-derived paths, per command. */
export function writeTargets(analysis: ShellAnalysis): WriteTarget[] {
  const targets = analysis.commands.flatMap((command) => {
    const found = argumentTargets(command.argv);
    const fromArgs =
      found === null
        ? []
        : found.paths.map((path) => ({ path, text: path ?? "", via: found.via, command }));
    return [...redirectTargets(command.redirects, command), ...fromArgs];
  });
  return [...targets, ...redirectTargets(analysis.compoundRedirects, null)];
}
