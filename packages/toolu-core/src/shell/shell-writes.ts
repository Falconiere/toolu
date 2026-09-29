/**
 * `@toolu/core/shell/writes` (#284): the files a command line writes. That is
 * output redirections on any command or compound command, and the file operands
 * of commands that write in place or copy. A dynamic target is reported with
 * `path: null` rather than dropped, and an unquoted pathname pattern
 * (`> .en[v]`, `cp x .e?v`) with its `pattern`: bash writes whichever existing
 * file the pattern matches. It is a separate entry from `@toolu/core/shell`, so
 * a bundle that never asks what a command writes does not carry it.
 */
import { basename } from "node:path";
import {
  hasOption,
  named,
  optionValues,
  parseArgs,
  type OptionSpec,
  type ParsedArgs,
} from "./shell-options.ts";
import type { ShellAnalysis, ShellCommand, ShellRedirect } from "./shell-types.ts";
import { stdinScript } from "./shell-words.ts";

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
type Target = Pick<WriteTarget, "path" | "pattern" | "text">;

/** `>`, `>>`, `>|`, `&>`, `&>>`, `<>`, and `>&` onto a file rather than a descriptor (`2>&1`, `2>&-`). */
function writesFile(redirect: ShellRedirect): boolean {
  if (redirect.operator === ">&") return !/^(\d+|-)$/.test(redirect.target ?? "");
  return /^(>>?|>\||&>>?|<>)$/.test(redirect.operator);
}

function argAt(command: ShellCommand, index: number): Target {
  const text = command.texts[index] ?? "";
  return { path: command.argv[index] ?? null, pattern: command.patterns[index] ?? null, text };
}

function operands(command: ShellCommand, parsed: ParsedArgs): Target[] {
  return parsed.operandAt.map((index) => argAt(command, index));
}

function join(dir: string | null, source: string | null): string | null {
  return dir === null || source === null ? null : `${dir.replace(/\/+$/, "")}/${basename(source)}`;
}

/** `source` copied into the directory `dir`. */
function inDir(dir: Target, source: Target): Target {
  const path = join(dir.path, source.path);
  const pattern =
    path === null ? join(dir.path ?? dir.pattern, source.path ?? source.pattern) : null;
  return { path, pattern, text: join(dir.text, source.text) ?? "" };
}

/** cp/mv/install: the destination and each source inside it, or each source inside `-t DIR`. */
function copyTargets(command: ShellCommand, spec: OptionSpec, installDirs = false): Target[] {
  const parsed = parseArgs(command.argv, 1, spec);
  const files = operands(command, parsed);
  if (installDirs && hasOption(parsed, "d directory")) return files;
  const dir = parsed.options.find((option) => named("t target-directory", option.name));
  if (dir !== undefined) {
    const value = dir.value ?? null;
    const into =
      dir.at === null ? { path: value, pattern: null, text: value ?? "" } : argAt(command, dir.at);
    return files.map((source) => inDir(into, source));
  }
  const dest = files.pop();
  if (dest === undefined || files.length === 0) return [];
  // DEST may be an existing directory (known only at run time), so each
  // DEST/basename(SRC) is a candidate too: `cp .env.example apps/web`.
  if (dest.path === null && dest.pattern === null) return [dest];
  return [dest, ...files.map((source) => inDir(dest, source))];
}

/** sed/perl edit files only in place; their first operand is the script unless `script` options gave one. */
function inPlaceTargets(command: ShellCommand, spec: OptionSpec, script: string): Target[] {
  const parsed = parseArgs(command.argv, 1, spec);
  if (!hasOption(parsed, "i in-place")) return [];
  // BSD `sed -i '' …`: the empty word is the backup suffix, not the script.
  const bsd = command.argv.findIndex((word, i) => word === "-i" && command.argv[i + 1] === "");
  const files = parsed.operandAt.filter((index) => bsd === -1 || index !== bsd + 1);
  return files.slice(hasOption(parsed, script) ? 0 : 1).map((index) => argAt(command, index));
}

/** An `open(` argument list that starts with a Python string literal: prefix, quote, body. */
const PY_PATH = /^\s*([rRbBuUfF]{0,2})('''|"""|'|")([\s\S]*?)\2\s*/;
/** The literal mode that follows the path, positional or `mode=`. */
const PY_MODE = /^,\s*(?:mode\s*=\s*)?[rRbBuUfF]{0,2}('''|"""|'|")([\s\S]*?)\1\s*[,)]/;
/** Other calls that write files; their targets are not read statically. */
const PY_WRITE_API =
  /\b(?:write_text|write_bytes|touch|symlink_to|hardlink_to|shutil\.(?:copy\w*|move)|os\.(?:rename|replace|symlink|link))\s*\(/;

/**
 * What one `open(` call writes: its static path; `null` when the path or mode
 * cannot be read statically (a variable, a concatenation, an f-string with a
 * field, a later keyword, a method such as `Path(…).open`); `undefined` for a read.
 */
function pythonOpen(args: string, method: boolean): string | null | undefined {
  const path = method ? null : PY_PATH.exec(args);
  if (path === null) return null;
  const rest = args.slice(path[0].length);
  if (rest.startsWith(")")) return undefined;
  const mode = PY_MODE.exec(rest)?.[2];
  if (mode === undefined) return null;
  if (!/[wax+]/.test(mode)) return undefined;
  const body = path[3] ?? "";
  return /[fF]/.test(path[1] ?? "") && body.includes("{") ? null : body;
}

/** The script python runs: `-c STRING`, or its stdin; `undefined` for a script file or module. */
function pythonScript(command: ShellCommand): string | null | undefined {
  const parsed = parseArgs(command.argv, 1, { valueShort: "cmWX", stopAtOperand: true });
  const [inline] = optionValues(parsed, "c");
  if (inline !== undefined) return inline;
  if (hasOption(parsed, "m")) return undefined;
  const operand = command.argv[parsed.next];
  return operand === undefined || operand === "-" ? stdinScript(command.redirects) : undefined;
}

/** Every file a python script writes; an unreadable script or write is one unknown target. */
function pythonTargets(command: ShellCommand): Target[] {
  const script = pythonScript(command);
  if (script === undefined) return [];
  const paths =
    script === null
      ? [null]
      : [...script.matchAll(/(\.?)\bopen\s*\(/g)].map((call) =>
          pythonOpen(script.slice(call.index + call[0].length), call[1] === "."),
        );
  if (script !== null && PY_WRITE_API.test(script)) paths.push(null);
  return paths.flatMap((path) =>
    path === undefined ? [] : [{ path, pattern: null, text: path ?? "" }],
  );
}

const MOVE: OptionSpec = { valueShort: "tS", valueLong: "target-directory suffix" };

/** The commands that write their operands, keyed by name. */
const WRITERS: Readonly<
  Record<string, { via: WriteVia; targets: (command: ShellCommand) => Target[] }>
> = {
  tee: { via: "tee", targets: (c) => operands(c, parseArgs(c.argv, 1, {})) },
  sed: {
    via: "sed",
    targets: (c) =>
      inPlaceTargets(
        c,
        { valueShort: "efl", restShort: "i", valueLong: "expression file line-length" },
        "e f expression file",
      ),
  },
  perl: {
    via: "perl",
    targets: (c) => inPlaceTargets(c, { valueShort: "eE", restShort: "iIMmlx0dDC" }, "e E"),
  },
  cp: { via: "cp", targets: (c) => copyTargets(c, MOVE) },
  mv: { via: "mv", targets: (c) => copyTargets(c, MOVE) },
  install: {
    via: "install",
    targets: (c) =>
      copyTargets(
        c,
        {
          valueShort: "tSmog",
          valueLong: "target-directory suffix mode owner group strip-program",
        },
        true,
      ),
  },
  // `of=…` as a whole never names an existing file, so it is not globbed.
  dd: {
    via: "dd",
    targets: (c) =>
      c.texts.flatMap((text, index) => {
        const word = c.argv[index] ?? c.patterns[index];
        const path = word?.startsWith("of=") === true ? word.slice(3) : null;
        return text.startsWith("of=") ? [{ path, pattern: null, text: text.slice(3) }] : [];
      }),
  },
  python: { via: "python", targets: pythonTargets },
};

function redirectTargets(
  redirects: readonly ShellRedirect[],
  command: ShellCommand | null,
): WriteTarget[] {
  return redirects.filter(writesFile).map(({ target, pattern, text }) => ({
    path: target,
    pattern,
    text,
    via: "redirect",
    command,
  }));
}

/** Every write target in the line: redirects first, then argument-derived paths, per command. */
export function writeTargets(analysis: ShellAnalysis): WriteTarget[] {
  const targets = analysis.commands.flatMap((command) => {
    const name = basename(command.argv[0] ?? "").replace(/^python[0-9.]*$/, "python");
    const writer = Object.hasOwn(WRITERS, name) ? WRITERS[name] : undefined;
    const fromArgs =
      writer === undefined
        ? []
        : writer
            .targets(command)
            .map(({ path, pattern, text }) => ({ path, pattern, text, via: writer.via, command }));
    return [...redirectTargets(command.redirects, command), ...fromArgs];
  });
  return [...targets, ...redirectTargets(analysis.compoundRedirects, null)];
}
